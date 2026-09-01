<script>
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { currentMonitor } from '@tauri-apps/api/window';
  import { open } from '@tauri-apps/plugin-dialog';

  const appWindow = getCurrentWebviewWindow();

  const DEFAULT_WIDTH = 340;
  const FALLBACK_COLLAPSED_WIDTH = 220;  // physical px — fallback jika pengukuran teks gagal
  const MIN_WIDTH = 160;
  const MAX_WIDTH = 640;
  const DEFAULT_HEIGHT = 260;   // PHYSICAL px — tinggi expanded default (eksis: EXPANDED_HEIGHT)
  const MIN_HEIGHT = 180;       // CSS px — floor usable, dikonversi × devicePixelRatio saat masuk jalur physical
  const MAX_HEIGHT = 520;       // PHYSICAL px — konsisten dengan resize_window (PhysicalSize)

  const STATUS_PRIORITY = ['error', 'waiting_confirmation', 'working', 'idle'];
  const STATUS_LABELS = {
    working: 'working',
    idle: 'idle',
    waiting_confirmation: 'waiting confirmation',
    error: 'error',
  };

  const FLASH_MS = 650;           // durasi animasi flash dot saat status berubah
  const HIGHLIGHT_WINDOW = 5000;  // jendela "baru berubah" (ms) untuk highlight saat expand
  const HIGHLIGHT_MS = 1200;      // durasi highlight item di expanded panel
  const MAX_STATUS_SEGMENTS = 3;  // maks segmen count per status di compact bar
  const NOTIFY_MS = 4000;         // durasi auto-expand notifikasi saat status berubah
  const SOUND_COOLDOWN_MS = 800;  // jarak minimal antar bunyi untuk status yang sama

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
    always_on_top: true, // deprecated: kept for compat, window always on top now
    auto_start: false,
    sounds_enabled: false,
    sounds: {},
    sound_loop: {},
  };

  let prevStates = new Map();      // pid -> state pada update sebelumnya
  let changedAt = new Map();       // pid -> timestamp status terakhir berubah
  let flashPids = new Set();       // pid yang sedang memicu animasi flash dot
  let highlightPids = [];          // pid yang sedang di-highlight di expanded panel
  let indicatorFlash = false;      // flash untuk indicator utama (compact bar)
  let prevAggStatus = null;        // aggregate status pada update sebelumnya
  let firstApply = true;           // hindari flash saat pertama kali data dimuat
  let notifyPending = false;       // auto-collapse notifikasi sedang berjalan
  let activeSounds = new Map();    // pid -> HTMLAudioElement yang sedang diputar
  let previewAudio = null;         // instance audio khusus preview di settings
  let lastSoundAt = {};            // pid:status -> timestamp bunyi terakhir (cooldown)

  $: count = agents.length;
  $: aggStatus = aggregateStatus(agents);
  $: statusCounts = countStatuses(agents);
  $: displayText = statusSummary(statusCounts);
  $: isExpanded = isLocked;
  $: if (!settings.sounds_enabled) stopAllSounds();

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

  function stopAgentSound(pid) {
    const audio = activeSounds.get(pid);
    if (!audio) return;
    try {
      audio.pause();
      audio.currentTime = 0;
    } catch (e) {
      /* ignore */
    }
    activeSounds.delete(pid);
  }

  function stopAllSounds() {
    for (const pid of Array.from(activeSounds.keys())) stopAgentSound(pid);
    if (previewAudio) {
      try {
        previewAudio.pause();
        previewAudio.currentTime = 0;
      } catch (e) {
        /* ignore */
      }
      previewAudio = null;
    }
  }

  function playStatusSound(pid, status, force = false) {
    if (!force && !settings.sounds_enabled) return;
    const path = settings.sounds?.[status];
    if (!path) return;
    const now = Date.now();
    const key = `${pid}:${status}`;
    if (!force && lastSoundAt[key] && now - lastSoundAt[key] < SOUND_COOLDOWN_MS) return;
    lastSoundAt[key] = now;
    stopAgentSound(pid);
    try {
      const audio = new Audio(convertFileSrc(path));
      audio.loop = !!settings.sound_loop?.[status];
      activeSounds.set(pid, audio);
      audio.play().catch(() => {
        if (activeSounds.get(pid) === audio) activeSounds.delete(pid);
      });
    } catch (e) {
      console.error('Sound playback failed:', e);
    }
  }

  function playPreviewSound(status) {
    const path = settings.sounds?.[status];
    if (!path) return;
    if (previewAudio) {
      try {
        previewAudio.pause();
        previewAudio.currentTime = 0;
      } catch (e) {
        /* ignore */
      }
    }
    try {
      const audio = new Audio(convertFileSrc(path));
      audio.loop = !!settings.sound_loop?.[status];
      previewAudio = audio;
      audio.play();
    } catch (e) {
      console.error('Sound preview failed:', e);
    }
  }

  function toggleLoop(status) {
    const next = { ...settings.sound_loop };
    next[status] = !settings.sound_loop?.[status];
    settings.sound_loop = next;
  }

  async function pickSound(status) {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: 'Audio', extensions: ['mp3', 'wav', 'ogg', 'm4a', 'flac'] },
          { name: 'All files', extensions: ['*'] },
        ],
      });
      if (typeof selected === 'string' && selected) {
        settings.sounds[status] = selected;
        settings.sounds = { ...settings.sounds };
      }
    } catch (e) {
      console.error('Failed to pick sound:', e);
    }
  }

  function clearSound(status) {
    if (!settings.sounds[status]) return;
    const next = { ...settings.sounds };
    delete next[status];
    settings.sounds = next;
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
        stopAgentSound(a.pid);
        playStatusSound(a.pid, a.state);
      }
    }
    for (const pid of Array.from(prevStates.keys())) {
      if (!nextPrev.has(pid)) {
        changedAt.delete(pid);
        stopAgentSound(pid);
      }
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
    // Jika masih collapsed, sesuaikan lebar dengan teks baru (fit-content)
    if (!isLocked && !isResizing && islandEl) {
      requestAnimationFrame(() => {
        const ideal = getCollapsedWidth();
        if (ideal !== currentWidth) {
          currentWidth = ideal;
          invoke('resize_window', { width: ideal, height: currentHeight, preserveCenterX: true, expandUp: expandUp }).catch(() => {});
        }
      });
    }
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
  let currentWidth = FALLBACK_COLLAPSED_WIDTH;    // live physical width — mulai collapsed (pill)
  let expandedWidth = DEFAULT_WIDTH;     // target lebar saat expanded (physical px) — session state
  let expandedHeight = DEFAULT_HEIGHT;   // target tinggi saat expanded (PHYSICAL px) — session state
  let expandUp = false;                  // expand ke atas (window terlalu dekat tepi bawah)

  function clamp(v, min, max) {
    return Math.max(min, Math.min(max, v));
  }

  async function computeExpandUp() {
    try {
      const monitor = await currentMonitor();
      if (!monitor) return false;
      const pos = await appWindow.outerPosition();
      const workBottom = monitor.workArea.position.y + monitor.workArea.size.height;
      return workBottom - pos.y < expandedHeight;
    } catch (e) {
      return false;
    }
  }

  async function resizeWindow(height, preserveCenterX = false, up = expandUp) {
    try {
      const applied = await invoke('resize_window', {
        width: currentWidth,
        height,
        preserveCenterX,
        expandUp: up,
      });
      currentHeight = applied ?? height;
      return applied ?? height;
    } catch (e) {
      console.error('Failed to resize window:', e);
      return height;
    }
  }

  async function resizeWindowGeneric(width, height, preserveCenterX = false, up = expandUp) {
    try {
      const applied = await invoke('resize_window', {
        width,
        height,
        preserveCenterX,
        expandUp: up,
      });
      currentWidth = width;
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

  function getIdealExpandedHeight() {
    const dpr = window.devicePixelRatio || 1;
    const barH = collapsedHeight(); // CSS px
    const minH = Math.max(
      Math.ceil(MIN_HEIGHT * dpr),
      Math.ceil((barH + 40) * dpr),
    );
    const EXTRA = 5; // breathing room agar scroll tidak muncul saat data sedikit
    let totalCSS;
    if (showSettings) {
      const sb = islandEl?.querySelector('.settings-body');
      let bodyH = 300; // fallback estimasi CSS — ditambah EXTRA nanti
      if (sb && sb.scrollHeight > 20) {
        if (sb.scrollHeight > 100 && sb.scrollHeight < 600) bodyH = sb.scrollHeight;
      }
      totalCSS = barH + 30 + 8 + bodyH + 8 + 12 + EXTRA;
    } else if (agents.length === 0) {
      const emptyH = 64;
      totalCSS = barH + 30 + 8 + emptyH + 8 + 12 + EXTRA;
    } else {
      const headerH = 30;
      const itemH = 52; // .agent-item ~52px (padding 16 + konten) — + EXTRA mencegah scroll tipis
      const gapItem = 4;
      const listH = agents.length * itemH + Math.max(0, agents.length - 1) * gapItem;
      totalCSS = barH + headerH + 8 + listH + 8 + 12 + EXTRA;
    }
    const idealPhysical = Math.ceil(totalCSS * dpr);
    return clamp(idealPhysical, minH, MAX_HEIGHT);
  }

  function getCollapsedWidth() {
    const dpr = window.devicePixelRatio || 1;
    const bar = islandEl?.querySelector('.compact-bar');
    if (!bar) return FALLBACK_COLLAPSED_WIDTH;
    const statusEl = bar.querySelector('.status-text');
    const indicator = bar.querySelector('.indicator');
    // scrollWidth = full content width walau overflow hidden (CSS px)
    const textW = statusEl ? statusEl.scrollWidth : 80;
    const indW = indicator ? indicator.offsetWidth : 8;
    const gap = 8;          // gap di .compact-bar
    const padding = 48;     // padding 24*2 di .compact-bar
    const safety = 16;      // breathing room agar tidak mepet
    const idealCSS = textW + indW + gap + padding + safety;
    const idealPhysical = Math.ceil(idealCSS * dpr);
    // collapsed tidak boleh lebih lebar dari expanded (expanded selalu lebih besar)
    const maxAllowed = Math.min(MAX_WIDTH, expandedWidth);
    return clamp(idealPhysical, MIN_WIDTH, maxAllowed);
  }

  function animateResize(from, to, duration, gen, up = expandUp) {
    return new Promise((resolve) => {
      const t0 = performance.now();
      const step = (now) => {
        if (gen !== undefined && gen !== generation) {
          resolve();
          return;
        }
        const p = Math.min(1, (now - t0) / duration);
        const ease = 1 - Math.pow(1 - p, 3);
        resizeWindow(Math.round(from + (to - from) * ease), false, up);
        if (p < 1) requestAnimationFrame(step);
        else resolve();
      };
      requestAnimationFrame(step);
    });
  }

  function animateWidth(from, to, duration, gen, up = expandUp) {
    return new Promise((resolve) => {
      const t0 = performance.now();
      const step = (now) => {
        if (gen !== undefined && gen !== generation) {
          resolve();
          return;
        }
        const p = Math.min(1, (now - t0) / duration);
        const ease = 1 - Math.pow(1 - p, 3);
        const w = Math.round(from + (to - from) * ease);
        currentWidth = w;
        // fire-and-forget preserveCenterX=true agar center-x tetap
        invoke('resize_window', { width: w, height: currentHeight, preserveCenterX: true, expandUp: up })
          .then((applied) => {
            if (applied != null) currentHeight = applied;
          })
          .catch(() => {});
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
    // sesuaikan tinggi dengan konten (auto-fit) sebelum expand
    expandedHeight = getIdealExpandedHeight();
    expandUp = await computeExpandUp();
    const now = Date.now();
    const recent = agents
      .filter((a) => changedAt.has(a.pid) && now - changedAt.get(a.pid) < HIGHLIGHT_WINDOW)
      .map((a) => a.pid);
    if (recent.length) addHighlight(recent);
    // Phase 1: perbesar width dulu (pill -> card) dengan center-x preservation
    const targetW = expandedWidth;
    if (currentWidth !== targetW) {
      await animateWidth(currentWidth, targetW, 280, gen, expandUp);
      if (gen !== generation) return;
    }
    // Phase 2: dropdown height
    await animateResize(currentHeight, expandedHeight, 320, gen, expandUp);
    if (gen !== generation) return;
    expandedHeight = await resizeWindowGeneric(targetW, expandedHeight, false, expandUp);
    currentWidth = targetW;
    showPanel = true;
  }

  function collapse() {
    if (!isLocked) return;
    const gen = ++generation;
    clearTimeout(phaseTimer);
    showPanel = false;
    showSettings = false;
    // Let the panel fade out, then shrink height, then shrink width sesuai teks.
    phaseTimer = setTimeout(async () => {
      if (gen !== generation) return;
      const h = collapsedHeight();
      const targetH = Math.ceil(h * window.devicePixelRatio);
      // Phase 1: height shrink
      await animateResize(currentHeight, targetH, 280, gen, expandUp);
      if (gen !== generation) return;
      // visual pill shape after height collapsed
      isLocked = false;
      // Phase 2: perkecil width (card -> pill) — ukuran pas dengan teks
      const targetW = getCollapsedWidth();
      if (currentWidth !== targetW) {
        await animateWidth(currentWidth, targetW, 260, gen, expandUp);
        if (gen !== generation) return;
        // final ensure window size exactly target (height + narrow width, center preserved)
        await resizeWindowGeneric(targetW, targetH, true, expandUp);
      } else {
        await resizeWindowGeneric(currentWidth, targetH, false, expandUp);
      }
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
    expandedHeight = getIdealExpandedHeight();
    expandUp = await computeExpandUp();
    // Width first then height, same as expand
    const targetW = expandedWidth;
    if (currentWidth !== targetW) {
      await animateWidth(currentWidth, targetW, 280, gen, expandUp);
      if (gen !== generation) return;
    }
    await animateResize(currentHeight, expandedHeight, 320, gen, expandUp);
    if (gen !== generation) return;
    expandedHeight = await resizeWindowGeneric(targetW, expandedHeight, false, expandUp);
    currentWidth = targetW;
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
    expandedWidth = newWidth;
    resizeWindow(currentHeight, true, expandUp);
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
    const delta = expandUp ? -dy : dy;   // up-mode: handle di tepi atas, seret ke atas = tinggi bertambah
    resizeStartHeight.pendingHeight = Math.round(clamp(
      resizeStartHeight.startHeight + delta,
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
    resizeWindow(newHeight, false, expandUp).then((applied) => {
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
    // width menyesuaikan teks (fit-content), height menyesuaikan bar
    await new Promise((r) => requestAnimationFrame(() => r()));
    currentWidth = getCollapsedWidth();
    await resizeWindowGeneric(currentWidth, Math.ceil(collapsedHeight() * window.devicePixelRatio), false, false);
  });

  onDestroy(() => {
    stopAllSounds();
    if (unlistenUpdate) unlistenUpdate();
    if (unlistenEvent) unlistenEvent();
    if (unlistenSettings) unlistenSettings();
  });
</script>

<div
  class="dynamic-island"
  class:expanded={isExpanded}
  class:expand-up={expandUp}
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
        <span>Poni Deck</span>
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
          <span class="setting-label">Jalankan saat PC menyala</span>
          <button
            class="toggle {settings.auto_start ? 'on' : ''}"
            on:click|stopPropagation={() => (settings.auto_start = !settings.auto_start)}
          ><span class="knob"></span></button>
        </div>
        <div class="setting-row">
          <span class="setting-label">Sounds</span>
          <button
            class="toggle {settings.sounds_enabled ? 'on' : ''}"
            on:click|stopPropagation={() => (settings.sounds_enabled = !settings.sounds_enabled)}
          ><span class="knob"></span></button>
        </div>
        <div class="sounds-section">
          <div class="sounds-title">Status sounds</div>
          {#each STATUS_PRIORITY as s}
            <div class="sound-row">
              <span class="sound-label">
                <span class="status-dot {s}"></span>
                {STATUS_LABELS[s] || s}
              </span>
              <span class="sound-path" title={settings.sounds[s] || ''}>
                {settings.sounds[s] ? formatPath(settings.sounds[s]) : 'none'}
              </span>
              <span class="sound-actions">
                <button class="mini-btn" on:click|stopPropagation={() => pickSound(s)}>Browse</button>
                <button
                  class="mini-btn"
                  disabled={!settings.sounds[s]}
                  on:click|stopPropagation={() => playPreviewSound(s)}
                >Play</button>
                <button
                  class="mini-btn"
                  class:active={!!settings.sound_loop[s]}
                  disabled={!settings.sounds[s]}
                  on:click|stopPropagation={() => toggleLoop(s)}
                >Loop</button>
                <button
                  class="mini-btn"
                  disabled={!settings.sounds[s]}
                  on:click|stopPropagation={() => clearSound(s)}
                >Clear</button>
              </span>
            </div>
          {/each}
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

  .dynamic-island.expand-up {
    flex-direction: column-reverse;
  }

  .dynamic-island.expand-up .compact-bar.expanded {
    border-radius: 0 0 8px 8px;
  }

  .dynamic-island.expand-up .expanded-panel {
    transform: translateY(8px);
  }

  .dynamic-island.expand-up .expanded-panel.visible {
    transform: translateY(0);
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

  .indicator.active.working {
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

  .dynamic-island.expand-up .resize-handle--bottom {
    bottom: auto;
    top: 0;
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

  .title-dot.working {
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

  .status-dot.working {
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

  .sounds-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sounds-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #777;
    padding: 2px 10px;
  }

  .sound-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 6px;
    min-width: 0;
  }

  .sound-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #ccc;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .sound-path {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    color: #666;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sound-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .mini-btn {
    background: rgba(255, 255, 255, 0.08);
    color: #bbb;
    border: none;
    border-radius: 5px;
    padding: 3px 8px;
    font-size: 10px;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .mini-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.16);
    color: #fff;
  }

  .mini-btn.active {
    background: rgba(74, 222, 128, 0.2);
    color: #4ade80;
  }

  .mini-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
