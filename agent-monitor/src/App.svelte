<script>
  import { onMount, onDestroy } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

  const appWindow = getCurrentWebviewWindow();

  let agents = [];
  let isLocked = false;
  let showPanel = false;
  let unlistenUpdate;
  let unlistenEvent;

  $: count = agents.length;
  $: displayText = count > 0 ? `${count} agent${count > 1 ? 's' : ''}` : 'idle';

  function formatPath(path) {
    if (!path) return 'unknown';
    if (path.length > 50) {
      return '...' + path.slice(-47);
    }
    return path;
  }

  function startDrag(e) {
    if (e.target.closest('[data-no-drag]')) return;
    appWindow.startDragging();
  }

  function toggleLock() {
    if (!isLocked) {
      isLocked = true;
      showPanel = true;
    } else {
      showPanel = false;
      setTimeout(() => { isLocked = false; }, 250);
    }
  }

  $: isExpanded = isLocked;

  async function openFolder(path) {
    try {
      const state = await invoke('get_config');
      await invoke('open_path', { path, action: state.click_action || 'terminal' });
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
  });

  onDestroy(() => {
    if (unlistenUpdate) unlistenUpdate();
    if (unlistenEvent) unlistenEvent();
  });
</script>

<div
  class="dynamic-island"
  class:expanded={isExpanded}
  class:locked={isLocked}
>
  <div class="compact-bar" class:expanded={isExpanded} on:mousedown={startDrag} on:click={toggleLock}>
    <span class="indicator" class:active={count > 0}></span>
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
            <div class="agent-item" on:click|stopPropagation={() => openFolder(agent.working_dir)}>
              <div class="agent-info">
                <span class="agent-pid">PID {agent.pid}</span>
                <span class="agent-path">{formatPath(agent.working_dir)}</span>
              </div>
              <div class="agent-status">
                <span class="status-dot running"></span>
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
    transition: clip-path 0.35s ease;
  }

  .dynamic-island:not(.expanded) {
    clip-path: inset(0 110px 220px 110px round 8px);
  }

  .compact-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 24px;
    min-height: 40px;
    transition: opacity 0.2s ease, border-radius 0.25s ease, background 0.25s ease, border 0.25s ease;
  }

  .compact-bar.expanded {
    border-radius: 8px 8px 0 0;
    border-bottom: none;
    background: transparent;
    backdrop-filter: none;
    border-color: transparent;
  }

  .indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #555;
    transition: background 0.3s ease;
  }

  .indicator.active {
    background: #4ade80;
    box-shadow: 0 0 6px rgba(74, 222, 128, 0.5);
  }

  .status-text {
    font-size: 13px;
    font-weight: 500;
    color: #ccc;
    white-space: nowrap;
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

  .badge {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
    padding: 2px 8px;
    border-radius: 8px;
    font-size: 11px;
    font-weight: 500;
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
  }

  .agent-item:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .agent-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
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
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .status-dot.running {
    background: #4ade80;
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
