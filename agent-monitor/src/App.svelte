<script>
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  const appWindow = getCurrentWebviewWindow();

  const DEFAULT_WIDTH = 340;
  const MIN_WIDTH = 160;
  const MAX_WIDTH = 640;
  const EXPANDED_HEIGHT = 260;

  const STATUS_PRIORITY = ['error', 'waiting_confirmation', 'working', 'running', 'idle'];
  const STATUS_LABELS = {
    working: 'working',
    idle: 'idle',
    waiting_confirmation: 'waiting confirmation',
    error: 'error',
    running: 'running',
  };

  let agents = [];
  let isLocked = false;
  let showPanel = false;
  let islandEl;
  let phaseTimer;
  let generation = 0;
  let unlistenUpdate;
  let unlistenEvent;

  $: count = agents.length;
  $: aggStatus = aggregateStatus(agents);
  $: displayText = count === 0 || aggStatus === 'idle'
    ? 'idle'
    : `${count} agent${count > 1 ? 's' : ''} · ${STATUS_LABELS[aggStatus] || aggStatus}`;
  $: isExpanded = isLocked;

  function aggregateStatus(list) {
    for (const status of STATUS_PRIORITY) {
      if (list.some((a) => a.state === status)) return status;
    }
    return 'idle';
  }

  function formatPath(path) {
    if (!path) return 'unknown';
    if (path.length > 50) {
      return '...' + path.slice(-47);
    }
    return path;
  }

  let currentHeight = EXPANDED_HEIGHT;
  let currentWidth = DEFAULT_WIDTH;

  function clamp(v, min, max) {
    return Math.max(min, Math.min(max, v));
  }

  async function resizeWindow(height, preserveCenterX = false) {
    currentHeight = height;
    try {
      await invoke('resize_window', {
        width: currentWidth,
        height,
        preserve_center_x: preserveCenterX,
      });
    } catch (e) {
      console.error('Failed to resize window:', e);
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
    await animateResize(currentHeight, EXPANDED_HEIGHT, 320, gen);
    if (gen !== generation) return;
    showPanel = true;
  }

  function collapse() {
    if (!isLocked) return;
    const gen = ++generation;
    clearTimeout(phaseTimer);
    showPanel = false;
    // Let the panel fade out, then shrink the window down to the bar.
    phaseTimer = setTimeout(() => {
      if (gen !== generation) return;
      const h = collapsedHeight();
      isLocked = false;
      animateResize(currentHeight, h, 320, gen);
    }, 250);
  }

  function toggleLock() {
    if (isResizing) return;
    if (isLocked) collapse();
    else expand();
  }

  let resizeStart = null;
  let isResizing = false;

  function startResize(e) {
    e.preventDefault();
    e.stopPropagation();
    if (isLocked) return;
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

  async function openFolder(agent) {
    try {
      await invoke('open_for_launcher', { path: agent.working_dir, launcher: agent.launcher ?? '' });
    } catch (e) {
      console.error('Failed to open path:', e);
    }
  }

  onMount(async () => {
    try {
      agents = await invoke('get_agents');
    } catch (e) {
      console.error('Failed to get agents:', e);
    }

    unlistenUpdate = await listen('agent-update', (event) => {
      agents = event.payload || [];
    });

    unlistenEvent = await listen('agent-event', (event) => {
      console.log('Agent event:', event.payload);
    });

    // Start collapsed: shrink the window so transparent area doesn't block clicks.
    await resizeWindow(collapsedHeight());
  });

  onDestroy(() => {
    if (unlistenUpdate) unlistenUpdate();
    if (unlistenEvent) unlistenEvent();
  });
</script>

<div
  class="dynamic-island"
  class:expanded={isExpanded}
  bind:this={islandEl}
>
  <div class="compact-bar" class:expanded={isExpanded} on:mousedown={startDrag} on:click={toggleLock}>
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
    <span class="indicator {aggStatus}" class:active={count > 0}></span>
    <span class="status-text">{displayText}</span>
  </div>

  <div class="expanded-panel" class:visible={showPanel}>
    <div class="panel-header">
      <span>Agent Monitor</span>
      <span class="badge">{count} active</span>
    </div>

    {#if agents.length === 0}
      <div class="empty-state">
        <span>No opencode agents detected</span>
        <span class="sub">Waiting for agent to start...</span>
      </div>
    {:else}
      <div class="agent-list">
          {#each agents as agent (agent.pid)}
            <div class="agent-item" on:click|stopPropagation={() => openFolder(agent)}>
              <div class="agent-info">
                <span class="agent-pid">PID {agent.pid}</span>
                <span class="agent-path">{formatPath(agent.working_dir)}</span>
              </div>
              <div class="agent-status">
                <span class="status-dot {agent.state}"></span>
                <span class="status-label">{STATUS_LABELS[agent.state] || agent.state}</span>
                {#if agent.launcher}
                  <span class="launcher-badge">{agent.launcher === 'vscode' ? 'VSCode' : 'Terminal'}</span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <div class="panel-footer">
        <button class="footer-btn" on:click|stopPropagation={() => invoke('open_terminal', { path: '' })?.catch(() => {})}>
          Open Terminal
        </button>
      </div>
  </div>
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

  .compact-bar:hover .resize-handle {
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

  .compact-bar.expanded .resize-handle {
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
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.5);
  }

  .indicator.active.working {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .indicator.active.idle {
    background: #6b7280;
  }

  .indicator.active.waiting_confirmation {
    background: #fbbf24;
    box-shadow: 0 0 6px rgba(251, 191, 36, 0.5);
  }

  .indicator.active.error {
    background: #ef4444;
    box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);
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
    display: flex;
    flex-direction: column;
    width: 100%;
    flex: 1;
    padding: 0 12px 12px;
    gap: 8px;
    opacity: 0;
    transform: translateY(-8px);
    transition: opacity 0.25s ease, transform 0.25s ease;
    pointer-events: none;
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

  .badge {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
    padding: 2px 8px;
    border-radius: 8px;
    font-size: 11px;
    font-weight: 500;
    flex-shrink: 0;
    white-space: nowrap;
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
    max-height: 140px;
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

  .agent-status {
    display: flex;
    align-items: center;
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
  }

  .status-dot.working {
    animation: pulse 1.2s ease-in-out infinite;
  }

  .status-dot.idle {
    background: #6b7280;
  }

  .status-dot.waiting_confirmation {
    background: #fbbf24;
  }

  .status-dot.error {
    background: #ef4444;
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

  .panel-footer {
    display: flex;
    justify-content: center;
    padding-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  .footer-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: #bbb;
    padding: 6px 16px;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .footer-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
  }
</style>
