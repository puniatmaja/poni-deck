<script>
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  const appWindow = getCurrentWebviewWindow();

  const DEFAULT_WIDTH = 340;
  const MIN_WIDTH = 160;
  const MAX_WIDTH = 640;
  const DEFAULT_HEIGHT = 260;   // PHYSICAL px — tinggi expanded default (eksis: EXPANDED_HEIGHT)
  const MIN_HEIGHT = 180;       // CSS px — floor usable, dikonversi × devicePixelRatio saat masuk jalur physical
  const MAX_HEIGHT = 520;       // PHYSICAL px — konsisten dengan resize_window (PhysicalSize)

  const STATUS_PRIORITY = ['error', 'waiting_confirmation', 'working', 'running', 'idle'];
  const STATUS_LABELS = {
    working: 'working',
    idle: 'idle',
    waiting_confirmation: 'waiting confirmation',
    error: 'error',
    running: 'running',
  };

  const FLASH_MS = 650;           // durasi animasi flash dot saat status berubah
  const HIGHLIGHT_WINDOW = 5000;  // jendela "baru berubah" (ms) untuk highlight saat expand
  const HIGHLIGHT_MS = 1200;      // durasi highlight item di expanded panel
  const MAX_STATUS_SEGMENTS = 3;  // maks segmen count per status di compact bar
  const NOTIFY_MS = 4000;         // durasi auto-expand notifikasi saat status berubah

  let agents = [];
  let isLocked = false;
  let showPanel = false;
  let showSettings = false;
  let saved = false;
  let islandEl;
  let phaseTimer;
  let notifyTimer;
  let generation = 0;
  let unlistenUpdate;
  let unlistenEvent;
  let unlistenSettings;
  let indicatorTimer;

  let settings = {
    polling_interval_ms: 2000,
    notifications_enabled: true,
    always_on_top: true,
    auto_start: false,
  };

  let prevStates = new Map();      // pid -> state pada update sebelumnya
  let changedAt = new Map();       // pid -> timestamp status terakhir berubah
  let flashPids = new Set();       // pid yang sedang memicu animasi flash dot
  let highlightPids = [];          // pid yang sedang di-highlight di expanded panel
  let indicatorFlash = false;      // flash untuk indicator utama (compact bar)
  let prevAggStatus = null;        // aggregate status pada update sebelumnya
  let firstApply = true;           // hindari flash saat pertama kali data dimuat
  let notifyPending = false;       // auto-collapse notifikasi sedang berjalan

  $: count = agents.length;
  $: aggStatus = aggregateStatus(agents);
  $: statusCounts = countStatuses(agents);
  $: displayText = statusSummary(statusCounts);
  $: isExpanded = isLocked;

  function aggregateStatus(list) {
    for (const status of STATUS_PRIORITY) {
      if (list.some((a) => a.state === status)) return status;
    }
    return 'idle';
  }

  function countStatuses(list) {
    const counts = {};
    for (const a of list) counts[a.state] = (counts[a.state] || 0) + 1;
    return counts;
  }

  function statusSummary(counts) {
    if (!counts || Object.keys(counts).length === 0) return 'idle';
    const parts = [];
    for (const s of STATUS_PRIORITY) {
      if (counts[s]) parts.push(`${counts[s]} ${STATUS_LABELS[s] || s}`);
    }
    if (parts.length > MAX_STATUS_SEGMENTS) {
      const rest = parts.length - MAX_STATUS_SEGMENTS + 1;
      return parts.slice(0, MAX_STATUS_SEGMENTS - 1).join(' · ') + ` · +${rest} more`;
    }
    return parts.join(' · ');
  }

  function applyAgents(next) {
    const now = Date.now();
    const nextPrev = new Map();
    const nextFlash = new Set();
    for (const a of next) {
      const prev = prevStates.get(a.pid);
      nextPrev.set(a.pid, a.state);
      if (prev !== undefined && prev !== a.state) {
        changedAt.set(a.pid, now);
        nextFlash.add(a.pid);
      }
    }
    for (const pid of Array.from(prevStates.keys())) {
      if (!nextPrev.has(pid)) changedAt.delete(pid);
    }
    prevStates = nextPrev;
    flashPids = nextFlash;

    const agg = aggregateStatus(next);
    indicatorFlash = !firstApply && agg !== prevAggStatus;
    prevAggStatus = agg;
    firstApply = false;

    if (indicatorFlash) {
      clearTimeout(indicatorTimer);
      indicatorTimer = setTimeout(() => {
        indicatorFlash = false;
      }, FLASH_MS);
    }
    if (nextFlash.size) {
      const pids = Array.from(nextFlash);
      if (showPanel) {
        addHighlight(pids);
      } else {
        const notifyPids = pids.filter((pid) => nextPrev.get(pid) !== 'working');
        if (notifyPids.length) notifyExpand();
      }
      setTimeout(() => {
        const nextSet = new Set(flashPids);
        for (const p of pids) nextSet.delete(p);
        flashPids = nextSet;
      }, FLASH_MS);
    }
    agents = next;
  }

  function notifyExpand() {
    if (isLocked) return;
    clearTimeout(notifyTimer);
    notifyPending = true;
    expand();
    notifyTimer = setTimeout(() => {
      if (!notifyPending) return;
      notifyPending = false;
      collapse();
    }, NOTIFY_MS);
  }

  function addHighlight(pids) {
    if (!pids.length) return;
    highlightPids = Array.from(new Set([...highlightPids, ...pids]));
    setTimeout(() => {
      const next = new Set(highlightPids);
      for (const p of pids) next.delete(p);
      highlightPids = Array.from(next);
    }, HIGHLIGHT_MS);
  }

  function formatPath(path) {
    if (!path) return 'unknown';
    if (path.length > 50) {
      return '...' + path.slice(-47);
    }
    return path;
  }

  let currentHeight = DEFAULT_HEIGHT;
  let currentWidth = DEFAULT_WIDTH;
  let expandedHeight = DEFAULT_HEIGHT;   // target tinggi saat expanded (PHYSICAL px) — session state

  function clamp(v, min, max) {
    return Math.max(min, Math.min(max, v));
  }

  async function resizeWindow(height, preserveCenterX = false) {
    try {
      const applied = await invoke('resize_window', {
        width: currentWidth,
        height,
        preserve_center_x: preserveCenterX,
      });
      currentHeight = applied ?? height;
      return applied ?? height;
    } catch (e) {
      console.error('Failed to resize window:', e);
      return height;
    }
  }

  function collapsedHeight() {
    const bar = islandEl?.querySelector('.compact-bar');
    return Math.ceil(bar?.getBoundingClientRect().height ?? 42);
  }

  function animateResize(from, to, duration, gen) {
    return new Promise((resolve) => {
      const t0 = performance.now();
      const step = (now) => {
        if (gen !== undefined && gen !== generation) {
          resolve();
          return;
        }
        const p = Math.min(1, (now - t0) / duration);
        const ease = 1 - Math.pow(1 - p, 3);
        resizeWindow(Math.round(from + (to - from) * ease));
        if (p < 1) requestAnimationFrame(step);
        else resolve();
      };
      requestAnimationFrame(step);
    });
  }

  let dragStart = null;

  function startDrag(e) {
    if (isResizing) return;
    if (e.target.closest('[data-no-drag]')) return;
    if (e.button !== 0) return;
    dragStart = { x: e.clientX, y: e.clientY };
    window.addEventListener('mousemove', onBarMove);
    window.addEventListener('mouseup', onBarUp);
  }

  function onBarMove(e) {
    if (!dragStart) return;
    if (Math.hypot(e.clientX - dragStart.x, e.clientY - dragStart.y) > 4) {
      cleanupDrag();
      appWindow.startDragging();
    }
  }

  function onBarUp() {
    cleanupDrag();
  }

  function cleanupDrag() {
    dragStart = null;
    window.removeEventListener('mousemove', onBarMove);
    window.removeEventListener('mouseup', onBarUp);
  }

  async function expand() {
    const gen = ++generation;
    clearTimeout(phaseTimer);
    isLocked = true;
    const now = Date.now();
    const recent = agents
      .filter((a) => changedAt.has(a.pid) && now - changedAt.get(a.pid) < HIGHLIGHT_WINDOW)
      .map((a) => a.pid);
    if (recent.length) addHighlight(recent);
    await animateResize(currentHeight, expandedHeight, 320, gen);
    if (gen !== generation) return;
    expandedHeight = await resizeWindow(expandedHeight);
    showPanel = true;
  }

  function collapse() {
    if (!isLocked) return;
    const gen = ++generation;
    clearTimeout(phaseTimer);
    showPanel = false;
    showSettings = false;
    // Let the panel fade out, then shrink the window down to the bar.
    phaseTimer = setTimeout(() => {
      if (gen !== generation) return;
      const h = collapsedHeight();
      isLocked = false;
      animateResize(currentHeight, Math.ceil(h * window.devicePixelRatio), 320, gen);
    }, 250);
  }

  function closeSettings() {
    collapse();
  }

  async function openSettings() {
    const gen = ++generation;
    clearTimeout(phaseTimer);
    clearTimeout(notifyTimer);
    notifyPending = false;
    isLocked = true;
    showSettings = true;
    await animateResize(currentHeight, expandedHeight, 320, gen);
    if (gen !== generation) return;
    expandedHeight = await resizeWindow(expandedHeight);
    showPanel = true;
  }

  async function saveSettings() {
    try {
      await invoke('set_config', { newConfig: settings });
      saved = true;
      setTimeout(() => (saved = false), 1500);
    } catch (e) {
      console.error('Failed to save settings:', e);
    }
  }

  function toggleLock() {
    if (isResizing) return;
    clearTimeout(notifyTimer);
    notifyPending = false;
    if (isLocked) collapse();
    else expand();
  }

  let resizeStart = null;
  let isResizing = false;

  function startResize(e) {
    e.preventDefault();
    e.stopPropagation();
    if (!isLocked) return;
    if (isResizing) return;
    isResizing = true;
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    resizeStart = {
      pointerId: e.pointerId,
      startClientX: e.clientX,
      startWidth: currentWidth,
      dir: handle.dataset.dir,
      pendingWidth: null,
      rafId: null,
    };
  }

  function onResizeMove(e) {
    if (!isResizing || !resizeStart || e.pointerId !== resizeStart.pointerId) return;
    const dx = (e.clientX - resizeStart.startClientX) * window.devicePixelRatio;
    const delta = resizeStart.dir === 'left' ? -dx : dx;
    resizeStart.pendingWidth = Math.round(clamp(resizeStart.startWidth + delta, MIN_WIDTH, MAX_WIDTH));
    if (resizeStart.rafId == null) {
      resizeStart.rafId = requestAnimationFrame(applyPendingWidth);
    }
  }

  function applyPendingWidth() {
    if (!isResizing || !resizeStart) return;
    resizeStart.rafId = null;
    const newWidth = resizeStart.pendingWidth;
    if (newWidth == null) return;
    if (newWidth === currentWidth) return;
    currentWidth = newWidth;
    resizeWindow(currentHeight, true);
  }

  function endResize(e) {
    if (!isResizing) return;
    if (resizeStart && resizeStart.rafId != null) {
      cancelAnimationFrame(resizeStart.rafId);
      resizeStart.rafId = null;
      applyPendingWidth();
    }
    if (resizeStart && e.currentTarget?.hasPointerCapture?.(resizeStart.pointerId)) {
      e.currentTarget.releasePointerCapture(resizeStart.pointerId);
    }
    isResizing = false;
    resizeStart = null;
  }

  let resizeStartHeight = null;

  function startResizeHeight(e) {
    e.preventDefault();
    e.stopPropagation();
    if (!isLocked || !showPanel) return;
    if (isResizing) return;
    isResizing = true;
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    resizeStartHeight = {
      pointerId: e.pointerId,
      startClientY: e.clientY,
      startHeight: expandedHeight,
      minH: Math.max(
        Math.ceil(MIN_HEIGHT * window.devicePixelRatio),
        Math.ceil((collapsedHeight() + 40) * window.devicePixelRatio),
      ),
      pendingHeight: null,
      rafId: null,
    };
  }

  function onResizeHeightMove(e) {
    if (!isResizing || !resizeStartHeight || e.pointerId !== resizeStartHeight.pointerId) return;
    const dy = (e.clientY - resizeStartHeight.startClientY) * window.devicePixelRatio;
    resizeStartHeight.pendingHeight = Math.round(clamp(
      resizeStartHeight.startHeight + dy,
      resizeStartHeight.minH,
      MAX_HEIGHT
    ));
    if (resizeStartHeight.rafId == null) {
      resizeStartHeight.rafId = requestAnimationFrame(applyPendingHeight);
    }
  }

  function applyPendingHeight() {
    if (!isResizing || !resizeStartHeight) return;
    resizeStartHeight.rafId = null;
    const newHeight = resizeStartHeight.pendingHeight;
    if (newHeight == null) return;
    if (newHeight === expandedHeight) return;
    expandedHeight = newHeight;
    resizeWindow(newHeight).then((applied) => {
      if (applied !== expandedHeight) expandedHeight = applied;
    });
  }

  function endResizeHeight(e) {
    if (!isResizing) return;
    if (resizeStartHeight && resizeStartHeight.rafId != null) {
      cancelAnimationFrame(resizeStartHeight.rafId);
      resizeStartHeight.rafId = null;
      applyPendingHeight();
    }
    if (resizeStartHeight && e.currentTarget?.hasPointerCapture?.(resizeStartHeight.pointerId)) {
      e.currentTarget.releasePointerCapture(resizeStartHeight.pointerId);
    }
    isResizing = false;
    resizeStartHeight = null;
  }

  async function openFolder(agent) {
    try {
      await invoke('open_for_launcher', { path: agent.working_dir, launcher: agent.launcher ?? '', pid: agent.pid });
    } catch (e) {
      console.error('Failed to open path:', e);
    }
  }

  onMount(async () => {
    try {
      applyAgents(await invoke('get_agents'));
    } catch (e) {
      console.error('Failed to get agents:', e);
    }

    try {
      settings = await invoke('get_config');
    } catch (e) {
      console.error('Failed to get config:', e);
    }

    unlistenUpdate = await listen('agent-update', (event) => {
      applyAgents(event.payload || []);
    });

    unlistenEvent = await listen('agent-event', (event) => {
      console.log('Agent event:', event.payload);
    });

    unlistenSettings = await listen('open-settings', () => {
      if (!isLocked) openSettings();
      else showSettings = true;
    });

    // Start collapsed: shrink the window so transparent area doesn't block clicks.
    await resizeWindow(Math.ceil(collapsedHeight() * window.devicePixelRatio));
  });

  onDestroy(() => {
    if (unlistenUpdate) unlistenUpdate();
    if (unlistenEvent) unlistenEvent();
    if (unlistenSettings) unlistenSettings();
  });
</script>

<div
  class="dynamic-island"
  class:expanded={isExpanded}
  bind:this={islandEl}
>
  <div class="compact-bar" class:expanded={isExpanded} on:mousedown={startDrag} on:click={toggleLock}>
    <span class="indicator {aggStatus}" class:active={count > 0} class:flash={indicatorFlash}></span>
    <span class="status-text">{displayText}</span>
  </div>

  <div class="expanded-panel" class:visible={showPanel}>
    <div class="panel-header">
      <span class="panel-title">
        <span class="title-dot {aggStatus}"></span>
        <span>Agent Monitor</span>
      </span>
      <span class="status-legend">
        {#each STATUS_PRIORITY as s}
          {#if statusCounts[s]}
            <span class="legend-item" title={STATUS_LABELS[s] || s}>
              <span class="status-dot {s}"></span>
              <span class="legend-count">{statusCounts[s]}</span>
            </span>
          {/if}
        {/each}
      </span>
    </div>

    {#if showSettings}
      <div class="settings-body">
        <div class="settings-header">
          <span>Settings</span>
          <button class="close-btn" data-no-drag on:click|stopPropagation={closeSettings}>&times;</button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Notifications</span>
          <button
            class="toggle {settings.notifications_enabled ? 'on' : ''}"
            on:click|stopPropagation={() => (settings.notifications_enabled = !settings.notifications_enabled)}
          ><span class="knob"></span></button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Always on top</span>
          <button
            class="toggle {settings.always_on_top ? 'on' : ''}"
            on:click|stopPropagation={() => (settings.always_on_top = !settings.always_on_top)}
          ><span class="knob"></span></button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Start with Windows</span>
          <button
            class="toggle {settings.auto_start ? 'on' : ''}"
            on:click|stopPropagation={() => (settings.auto_start = !settings.auto_start)}
          ><span class="knob"></span></button>
        </div>
        <div class="settings-actions">
          <button class="save-btn" on:click|stopPropagation={saveSettings}>Save</button>
          <span class="saved-msg" class:visible={saved}>Saved</span>
        </div>
      </div>
    {:else if agents.length === 0}
      <div class="empty-state">
        <span>No agents detected</span>
        <span class="sub">Waiting for agent to start...</span>
      </div>
    {:else}
      <div class="agent-list">
          {#each agents as agent (agent.pid)}
            <div class="agent-item" class:highlighted={highlightPids.includes(agent.pid)} on:click|stopPropagation={() => openFolder(agent)}>
              <div class="agent-info">
                <span class="agent-pid">PID {agent.pid}</span>
                <span class="agent-path">{formatPath(agent.working_dir)}</span>
              </div>
              <div class="agent-right">
                <div class="agent-status">
                  <span class="status-dot {agent.state}" class:flash={flashPids.has(agent.pid)}></span>
                  <span class="status-label">{STATUS_LABELS[agent.state] || agent.state}</span>
                  {#if agent.launcher}
                    <span class="launcher-badge">{agent.launcher === 'vscode' ? 'VSCode' : 'Terminal'}</span>
                  {/if}
                </div>
                {#if agent.tool}
                  <span class="tool-badge">{agent.tool === 'claude' ? 'Claude' : 'opencode'}</span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <span class="resize-handle--bottom"
            data-no-drag
            on:pointerdown={startResizeHeight}
            on:pointermove={onResizeHeightMove}
            on:pointerup={endResizeHeight}
            on:pointercancel={endResizeHeight}
            on:lostpointercapture={endResizeHeight}
            on:click|stopPropagation
            on:contextmenu|preventDefault></span>
  </div>

  <span class="resize-handle resize-handle--left"
        data-no-drag
        data-dir="left"
        on:pointerdown={startResize}
        on:pointermove={onResizeMove}
        on:pointerup={endResize}
        on:pointercancel={endResize}
        on:lostpointercapture={endResize}
        on:click|stopPropagation
        on:contextmenu|preventDefault></span>
  <span class="resize-handle resize-handle--right"
        data-no-drag
        data-dir="right"
        on:pointerdown={startResize}
        on:pointermove={onResizeMove}
        on:pointerup={endResize}
        on:pointercancel={endResize}
        on:lostpointercapture={endResize}
        on:click|stopPropagation
        on:contextmenu|preventDefault></span>
</div>

<style>
  html, body {
    margin: 0;
    padding: 0;
    background: transparent;
  }

  .dynamic-island {
    display: flex;
    flex-direction: column;
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    cursor: default;
    user-select: none;
    background: rgba(30, 30, 35, 0.95);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
  }

  .compact-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 24px;
    min-height: 40px;
    position: relative;
    transition: opacity 0.2s ease, border-radius 0.25s ease, background 0.25s ease, border 0.25s ease;
  }

  .compact-bar.expanded {
    border-radius: 8px 8px 0 0;
    border-bottom: none;
    background: transparent;
    backdrop-filter: none;
    border-color: transparent;
  }

  .resize-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 10px;
    cursor: ew-resize;
    touch-action: none;
    z-index: 5;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .dynamic-island:hover .resize-handle {
    opacity: 0.5;
  }

  .resize-handle:hover,
  .resize-handle:active {
    opacity: 1;
  }

  .resize-handle--left {
    left: 0;
  }

  .resize-handle--right {
    right: 0;
  }

  .dynamic-island:not(.expanded) .resize-handle {
    display: none;
  }

  .indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
    transition: background 0.3s ease, box-shadow 0.3s ease;
  }

  .indicator.active.working,
  .indicator.active.running {
    background: #4ade80;
    --dot-color: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.5);
  }

  .indicator.active.working {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .indicator.active.idle {
    background: #6b7280;
    --dot-color: #6b7280;
  }

  .indicator.active.waiting_confirmation {
    background: #fbbf24;
    --dot-color: #fbbf24;
    box-shadow: 0 0 6px rgba(251, 191, 36, 0.5);
  }

  .indicator.active.error {
    background: #ef4444;
    --dot-color: #ef4444;
    box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);
  }

  @keyframes statusFlash {
    0% { box-shadow: 0 0 0 0 var(--dot-color, #fff); opacity: 1; }
    60% { opacity: 0.35; }
    100% { box-shadow: 0 0 0 7px rgba(255, 255, 255, 0); opacity: 1; }
  }

  .indicator.flash,
  .status-dot.flash {
    animation: statusFlash 0.65s ease-out;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }

  .status-text {
    font-size: 13px;
    font-weight: 500;
    color: #ccc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .expanded-panel {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    min-height: 0;
    padding: 0 12px 12px;
    gap: 8px;
    opacity: 0;
    transform: translateY(-8px);
    transition: opacity 0.25s ease, transform 0.25s ease;
    pointer-events: none;
  }

  .expanded-panel:not(.visible) .resize-handle--bottom {
    display: none;
  }

  .resize-handle--bottom {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 10px;
    cursor: ns-resize;
    touch-action: none;
    z-index: 5;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .expanded-panel:hover .resize-handle--bottom {
    opacity: 0.5;
  }

  .resize-handle--bottom:hover,
  .resize-handle--bottom:active {
    opacity: 1;
  }

  .expanded-panel.visible {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    font-weight: 600;
    color: #fff;
    padding: 4px 4px 8px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .panel-header > span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .panel-title {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .title-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
    flex-shrink: 0;
    transition: background 0.3s ease, box-shadow 0.3s ease;
  }

  .title-dot.working,
  .title-dot.running {
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.5);
  }

  .title-dot.working {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .title-dot.idle {
    background: #6b7280;
  }

  .title-dot.waiting_confirmation {
    background: #fbbf24;
    box-shadow: 0 0 6px rgba(251, 191, 36, 0.5);
  }

  .title-dot.error {
    background: #ef4444;
    box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);
  }

  .status-legend {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .legend-count {
    font-size: 11px;
    font-weight: 600;
    color: #ccc;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 24px 0;
    color: #888;
    font-size: 13px;
  }

  .sub {
    font-size: 11px;
    color: #666;
  }

  .agent-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
  }

  .agent-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 10px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.2s;
    min-width: 0;
    gap: 8px;
  }

  .agent-item:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  @keyframes itemFlash {
    0% { background: rgba(255, 255, 255, 0.22); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.4); }
    100% { background: rgba(255, 255, 255, 0.04); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0); }
  }

  .agent-item.highlighted {
    animation: itemFlash 1.2s ease-out;
  }

  .agent-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    min-width: 0;
  }

  .agent-pid {
    font-size: 12px;
    font-weight: 600;
    color: #aaa;
  }

  .agent-path {
    font-size: 11px;
    color: #777;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 240px;
  }

  .agent-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: 4px;
    min-width: 0;
  }

  .agent-status {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    min-width: 0;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #555;
    transition: background 0.3s ease;
  }

  .status-dot.working,
  .status-dot.running {
    background: #4ade80;
    --dot-color: #4ade80;
  }

  .status-dot.working {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .status-dot.idle {
    background: #6b7280;
    --dot-color: #6b7280;
  }

  .status-dot.waiting_confirmation {
    background: #fbbf24;
    --dot-color: #fbbf24;
  }

  .status-dot.error {
    background: #ef4444;
    --dot-color: #ef4444;
  }

  .status-label {
    font-size: 11px;
    color: #aaa;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .launcher-badge {
    font-size: 10px;
    color: #888;
    background: rgba(255, 255, 255, 0.06);
    padding: 1px 6px;
    border-radius: 6px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .tool-badge {
    font-size: 10px;
    font-weight: 600;
    color: #e6a23c;
    background: rgba(230, 162, 60, 0.14);
    padding: 1px 6px;
    border-radius: 6px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .settings-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1 1 auto;
    min-height: 0;
    padding: 8px 4px;
    overflow-y: auto;
  }

  .settings-header {
    display: flex;
    justify-content: center;
    align-items: center;
    position: relative;
    padding: 2px 4px 6px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 13px;
    font-weight: 600;
    color: #fff;
  }

  .close-btn {
    position: absolute;
    right: 0;
    top: 0;
    background: none;
    border: none;
    color: #888;
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .close-btn:hover {
    color: #fff;
  }

  .setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 6px;
  }

  .setting-label {
    font-size: 12px;
    color: #ccc;
  }

  .toggle {
    position: relative;
    width: 34px;
    height: 20px;
    border-radius: 10px;
    border: none;
    background: #444;
    cursor: pointer;
    padding: 0;
    transition: background 0.2s ease;
    flex-shrink: 0;
  }

  .toggle .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #aaa;
    transition: transform 0.2s ease, background 0.2s ease;
  }

  .toggle.on {
    background: #4ade80;
  }

  .toggle.on .knob {
    transform: translateX(14px);
    background: #fff;
  }

  .settings-actions {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 2px 4px;
  }

  .save-btn {
    background: rgba(74, 222, 128, 0.2);
    color: #4ade80;
    border: 1px solid rgba(74, 222, 128, 0.4);
    border-radius: 6px;
    padding: 6px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s ease;
  }

  .save-btn:hover {
    background: rgba(74, 222, 128, 0.3);
  }

  .saved-msg {
    position: absolute;
    right: 12px;
    font-size: 11px;
    color: #4ade80;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .saved-msg.visible {
    opacity: 1;
  }
</style>
