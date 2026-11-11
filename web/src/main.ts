import './style.css';
import { initCesiumGlobe, BasemapType } from './globe/cesium';
import { EntityLayerManager } from './globe/entities';
import { initTelemetryChart, TelemetryChartManager } from './charts/telemetry';
import { SnapshotComparisonUI } from './charts/comparison';
import {
  fetchHealth,
  fetchObservations,
  DataPoint,
  fetchAuthStatus,
  setLanPassword,
  setLanMode,
  fetchStorageStatus,
  clearCaches,
  deleteAllSnapshots,
  compactStorage,
  fetchAlertRules,
  testToast,
  testDiscordWebhook,
  setVaultSecret,
  fetchAttributions,
  ProviderAttribution,
  fetchVaultKeys,
  testProviderKey,
  deleteProviderKey,
  shutdownApp,
  fetchBoundaries,
  fetchSubmarineCables,
} from './services/api';
import { telemetryWs } from './services/websocket';

/**
 * Main application bootstrap for HUMBOLDT.
 */
async function bootstrap() {
  console.log('[HUMBOLDT] Initializing terminal workstation...');

  // 1. Start real-time UTC clock in top bar
  const clockEl = document.getElementById('utc-clock');
  const updateUtcClock = () => {
    if (clockEl) {
      const now = new Date();
      clockEl.textContent = now.toISOString().substring(11, 19) + ' UTC';
    }
  };
  updateUtcClock();
  setInterval(updateUtcClock, 1000);

  // 2. Query server health and active port
  const portEl = document.getElementById('current-port');
  const healthEl = document.getElementById('current-health');
  try {
    const health = await fetchHealth();
    if (portEl) portEl.textContent = health.port.toString();
    if (healthEl) healthEl.textContent = health.status.toUpperCase();
  } catch (err) {
    console.warn('[HUMBOLDT] Backend health probe failed:', err);
  }

  // 3. Initialize Cesium 3D Globe with Tier 1 offline tiles
  let entityLayers: EntityLayerManager | null = null;
  try {
    const globe = await initCesiumGlobe('cesiumContainer');
    entityLayers = new EntityLayerManager(globe.viewer);
    console.log('[HUMBOLDT] 3D Globe & Entity Subsystems initialized.');

    // Wire up Basemap Resolution selector controls
    const bindBasemapSelector = (selectId: string) => {
      const select = document.getElementById(selectId) as HTMLSelectElement | null;
      if (!select) return;
      select.value = globe.getBasemap();
      select.addEventListener('change', async () => {
        const val = select.value as BasemapType;
        await globe.setBasemap(val);
        const otherId = selectId === 'select-basemap' ? 'quick-select-basemap' : 'select-basemap';
        const otherSelect = document.getElementById(otherId) as HTMLSelectElement | null;
        if (otherSelect && otherSelect.value !== val) {
          otherSelect.value = val;
        }
      });
    };

    bindBasemapSelector('select-basemap');
    bindBasemapSelector('quick-select-basemap');

    try {
      const boundaries = await fetchBoundaries();
      entityLayers.renderBoundaries(boundaries);
      console.log(`[HUMBOLDT] Rendered boundaries: ${boundaries.countries.length} countries, ${boundaries.states.length} states, ${boundaries.counties.length} counties, ${boundaries.places.length} places.`);
    } catch (err) {
      console.warn('[HUMBOLDT] Failed to load boundaries:', err);
    }
  } catch (err) {
    console.error('[HUMBOLDT] Failed to initialize 3D Globe:', err);
  }

  // 4. Connect to Live WebSocket Observation Bus
  const wsStatusEl = document.getElementById('ws-status');
  const wsIndicatorEl = document.getElementById('ws-indicator');
  telemetryWs.onStatusChange((connected) => {
    if (wsStatusEl) {
      wsStatusEl.textContent = connected ? 'LIVE STREAM' : 'RECONNECTING';
    }
    if (wsIndicatorEl) {
      if (connected) {
        wsIndicatorEl.classList.remove('status-alert');
        wsIndicatorEl.classList.add('status-nominal');
      } else {
        wsIndicatorEl.classList.remove('status-nominal');
        wsIndicatorEl.classList.add('status-alert');
      }
    }
  });

  telemetryWs.subscribe((obs) => {
    if (entityLayers) {
      entityLayers.handleObservation(obs);
    }
  });

  // 5. Ingest initial telemetry across all layers
  const refreshLayers = async () => {
    if (!entityLayers) return;

    try {
      const [
        quakes,
        satellites,
        aircraft,
        ships,
        weather,
        spaceWeather,
        powerGrid,
        wildfires,
        airQuality,
        hydro,
        buoys,
        cyclones,
        volcanoes,
        radiation,
        aurora,
        disasters,
        cables,
      ] = await Promise.all([
        fetchObservations('quake', 100).catch(() => []),
        fetchObservations('satellite', 100).catch(() => []),
        fetchObservations('aircraft', 100).catch(() => []),
        fetchObservations('ship', 100).catch(() => []),
        fetchObservations('weather', 50).catch(() => []),
        fetchObservations('space_weather', 10).catch(() => []),
        fetchObservations('power_grid', 10).catch(() => []),
        fetchObservations('wildfire', 100).catch(() => []),
        fetchObservations('air_quality', 100).catch(() => []),
        fetchObservations('hydro', 100).catch(() => []),
        fetchObservations('buoy', 100).catch(() => []),
        fetchObservations('cyclone', 50).catch(() => []),
        fetchObservations('volcano', 50).catch(() => []),
        fetchObservations('radiation', 100).catch(() => []),
        fetchObservations('aurora', 100).catch(() => []),
        fetchObservations('disaster', 50).catch(() => []),
        fetchSubmarineCables().catch(() => []),
      ]);

      if (quakes.length > 0) {
        entityLayers.renderQuakes(quakes);
        console.log(`[HUMBOLDT] Rendered ${quakes.length} earthquakes.`);
      }

      if (satellites.length > 0) {
        entityLayers.renderSatellites(satellites);
        console.log(`[HUMBOLDT] Rendered ${satellites.length} satellites.`);
      }

      if (aircraft.length > 0) {
        entityLayers.renderAircraft(aircraft);
        console.log(`[HUMBOLDT] Rendered ${aircraft.length} aircraft.`);
      }

      if (ships.length > 0) {
        entityLayers.renderShips(ships);
        console.log(`[HUMBOLDT] Rendered ${ships.length} vessels.`);
      }

      for (const w of weather) {
        entityLayers.upsertWeather(w);
      }

      if (wildfires.length > 0) {
        entityLayers.renderWildfires(wildfires);
        console.log(`[HUMBOLDT] Rendered ${wildfires.length} wildfires.`);
      }

      if (airQuality.length > 0) {
        entityLayers.renderAirQuality(airQuality);
        console.log(`[HUMBOLDT] Rendered ${airQuality.length} air quality stations.`);
      }

      if (hydro.length > 0) {
        entityLayers.renderHydro(hydro);
        console.log(`[HUMBOLDT] Rendered ${hydro.length} river gauges.`);
      }

      if (buoys.length > 0) {
        entityLayers.renderBuoys(buoys);
        console.log(`[HUMBOLDT] Rendered ${buoys.length} marine buoys.`);
      }

      if (cyclones.length > 0) {
        entityLayers.renderCyclones(cyclones);
        console.log(`[HUMBOLDT] Rendered ${cyclones.length} tropical cyclones.`);
      }

      if (volcanoes.length > 0) {
        entityLayers.renderVolcanoes(volcanoes);
        console.log(`[HUMBOLDT] Rendered ${volcanoes.length} active volcanoes.`);
      }

      if (radiation.length > 0) {
        entityLayers.renderRadiation(radiation);
        console.log(`[HUMBOLDT] Rendered ${radiation.length} radiation monitors.`);
      }

      if (aurora.length > 0) {
        entityLayers.renderAurora(aurora);
        console.log(`[HUMBOLDT] Rendered ${aurora.length} auroral oval coordinates.`);
      }

      if (disasters.length > 0) {
        entityLayers.renderDisasters(disasters);
        console.log(`[HUMBOLDT] Rendered ${disasters.length} crisis disaster events.`);
      }

      if (cables.length > 0) {
        entityLayers.renderSubmarineCables(cables);
        console.log(`[HUMBOLDT] Rendered ${cables.length} subsea fiber optic cables.`);
      }

      if (spaceWeather.length > 0) {
        entityLayers.updateSpaceWeatherDisplay(spaceWeather[0]);
      }

      if (powerGrid.length > 0) {
        entityLayers.updatePowerGridDisplay(powerGrid[0]);
      }
    } catch (err) {
      console.warn('[HUMBOLDT] Layer initial fetch error:', err);
    }
  };

  await refreshLayers();
  // Poll backup every 45 seconds
  setInterval(refreshLayers, 45000);

  // 6. Initialize ECharts Telemetry Widget & Snapshot Comparison UI
  let telemetryChart: TelemetryChartManager | null = null;
  let comparisonUI: SnapshotComparisonUI | null = null;
  let activeLivePoints: DataPoint[] = [
    { timestamp: Date.now() - 3600000 * 6, value: 2.1 },
    { timestamp: Date.now() - 3600000 * 5, value: 2.4 },
    { timestamp: Date.now() - 3600000 * 4, value: 2.0 },
    { timestamp: Date.now() - 3600000 * 3, value: 3.2 },
    { timestamp: Date.now() - 3600000 * 2, value: 2.8 },
    { timestamp: Date.now() - 3600000 * 1, value: 3.0 },
    { timestamp: Date.now(), value: 2.7 },
  ];

  try {
    telemetryChart = initTelemetryChart('echarts-telemetry');
    comparisonUI = new SnapshotComparisonUI(
      'echarts-comparison',
      'variance-banner',
      'snapshot-select-a',
      'snapshot-select-b',
      'btn-comp-mode'
    );
    await comparisonUI.initialize();
    console.log('[HUMBOLDT] Telemetry & Comparison subsystems initialized.');
  } catch (err) {
    console.error('[HUMBOLDT] Failed to initialize charts:', err);
  }

  // Subpanel Tab Switching (Live vs Compare)
  const btnTabLive = document.getElementById('btn-tab-live');
  const btnTabCompare = document.getElementById('btn-tab-compare');
  const subpanelLive = document.getElementById('subpanel-live');
  const subpanelCompare = document.getElementById('subpanel-compare');

  if (btnTabLive && btnTabCompare && subpanelLive && subpanelCompare) {
    btnTabLive.addEventListener('click', () => {
      btnTabLive.classList.add('active');
      btnTabCompare.classList.remove('active');
      subpanelLive.style.display = 'flex';
      subpanelCompare.style.display = 'none';
      telemetryChart?.resize();
    });

    btnTabCompare.addEventListener('click', async () => {
      btnTabCompare.classList.add('active');
      btnTabLive.classList.remove('active');
      subpanelCompare.style.display = 'flex';
      subpanelLive.style.display = 'none';
      if (comparisonUI) {
        await comparisonUI.refreshSnapshotList();
        comparisonUI.resize?.();
      }
    });
  }

  // Freeze Snapshot Button
  const btnSaveSnapshot = document.getElementById('btn-save-snapshot');
  if (btnSaveSnapshot && comparisonUI) {
    btnSaveSnapshot.addEventListener('click', async () => {
      const defaultName = `Snapshot ${new Date().toISOString().substring(0, 16).replace('T', ' ')}`;
      const name = window.prompt('Enter label for immutable snapshot:', defaultName);
      if (!name) return;

      try {
        await comparisonUI.freezeSnapshotFromLive(
          name,
          'space_weather',
          'kp_index',
          activeLivePoints
        );
        alert(`Snapshot '${name}' frozen permanently.`);
      } catch (err) {
        console.error('[HUMBOLDT] Snapshot freeze failed:', err);
        alert('Failed to save snapshot: ' + err);
      }
    });
  }

  // Export Buttons
  const btnExportPng = document.getElementById('btn-export-png');
  const btnExportCsv = document.getElementById('btn-export-csv');
  if (btnExportPng && comparisonUI) {
    btnExportPng.addEventListener('click', () => {
      comparisonUI?.exportCurrentPng();
    });
  }
  if (btnExportCsv && comparisonUI) {
    btnExportCsv.addEventListener('click', () => {
      comparisonUI?.exportSelectedCsv();
    });
  }

  // 7. Wire up Layer Rail checkbox listeners
  const layerInputs = document.querySelectorAll<HTMLInputElement>(
    '#layer-toggles input[type="checkbox"]'
  );
  layerInputs.forEach((input) => {
    input.addEventListener('change', () => {
      const layer = input.getAttribute('data-layer');
      if (layer && entityLayers) {
        entityLayers.setLayerVisibility(layer, input.checked);
      }
      console.log(`[HUMBOLDT] Layer '${layer}' visibility: ${input.checked}`);
    });
  });

  // Check vault keys and update notices for key-gated layers
  await refreshLayerRailKeyStatuses();

  // 8. Wire up Timeline Bar (Play/Pause, Step, Speed Controls)
  const btnPlayPause = document.getElementById('btn-play-pause');
  const playPauseIcon = document.getElementById('play-pause-icon');
  const btnStepFwd = document.getElementById('btn-step-fwd');
  const currentSpeedEl = document.getElementById('current-speed');
  const liveIndicatorEl = document.querySelector('.timeline-live-indicator');

  if (btnPlayPause && playPauseIcon) {
    playPauseIcon.textContent = '⏸';
    btnPlayPause.addEventListener('click', () => {
      if (!entityLayers) return;
      const isPaused = !entityLayers.isPaused();
      entityLayers.setSimulationPaused(isPaused);
      playPauseIcon.textContent = isPaused ? '▶' : '⏸';
      if (liveIndicatorEl) {
        liveIndicatorEl.classList.toggle('paused', isPaused);
      }
    });
  }

  const speeds = [1, 2, 5, 10];
  let speedIdx = 0;
  if (currentSpeedEl) {
    currentSpeedEl.style.cursor = 'pointer';
    currentSpeedEl.title = 'Click to toggle simulation speed (1x, 2x, 5x, 10x)';
    currentSpeedEl.addEventListener('click', () => {
      if (!entityLayers) return;
      speedIdx = (speedIdx + 1) % speeds.length;
      const speed = speeds[speedIdx];
      entityLayers.setSimulationSpeed(speed);
      currentSpeedEl.textContent = `${speed}x`;
    });
  }

  if (btnStepFwd) {
    btnStepFwd.addEventListener('click', () => {
      if (!entityLayers) return;
      entityLayers.setSimulationPaused(false);
      setTimeout(() => {
        if (playPauseIcon && playPauseIcon.textContent === '▶') {
          entityLayers?.setSimulationPaused(true);
        }
      }, 200);
    });
  }

  // 9. Wire up Settings & LAN Security modal
  initSettingsModal();

  // 10. Wire up Attribution & Open Licenses modal
  initAttributionModal();

  // 11. Wire up Application Quit action
  const handleQuit = async () => {
    if (!confirm('Are you sure you want to terminate HUMBOLDT?')) return;
    try {
      await shutdownApp();
      document.body.innerHTML = `
        <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100vh;background:#0d1117;color:#c9d1d9;font-family:monospace;text-align:center;padding:24px;">
          <h2 style="color:#58a6ff;margin-bottom:12px;letter-spacing:1px;">◈ HUMBOLDT WORKSTATION TERMINATED</h2>
          <p style="color:#8b949e;max-width:480px;line-height:1.5;font-size:12px;">The daemon process has terminated cleanly and released all active network ports. You may safely close this browser window.</p>
        </div>
      `;
    } catch (err) {
      alert('Failed to send termination signal: ' + err);
    }
  };
  document.getElementById('btn-quit')?.addEventListener('click', handleQuit);
  document.getElementById('btn-quit-settings')?.addEventListener('click', handleQuit);

  document.getElementById('btn-open-keys-from-settings')?.addEventListener('click', () => {
    const settingsModal = document.getElementById('settings-modal');
    if (settingsModal) settingsModal.style.display = 'none';
    const attrModal = document.getElementById('attribution-modal');
    if (attrModal) {
      attrModal.style.display = 'flex';
      const btnAttr = document.getElementById('btn-attribution');
      btnAttr?.click();
    }
  });
}

/**
 * Manages the settings modal, LAN access gating, notification tests, and storage tools.
 */
function initSettingsModal() {
  const btnSettings = document.getElementById('btn-settings');
  const btnCloseSettings = document.getElementById('btn-close-settings');
  const modal = document.getElementById('settings-modal');

  const updateSettingsView = async () => {
    // 1. Auth Status
    try {
      const auth = await fetchAuthStatus();
      const badge = document.getElementById('lan-status-badge');
      const toggle = document.getElementById('toggle-lan-mode') as HTMLInputElement | null;
      const toggleLabel = document.getElementById('lan-toggle-label');
      if (badge) {
        badge.textContent = auth.lan_enabled ? 'LAN ACTIVE' : 'LOCAL LOOPBACK';
      }
      if (toggle) {
        toggle.checked = auth.lan_enabled;
      }
      if (toggleLabel) {
        toggleLabel.textContent = auth.lan_enabled ? 'Enabled' : 'Disabled';
      }
    } catch (e) {
      console.warn('Auth status fetch failed:', e);
    }

    // 2. Storage Status
    try {
      const storage = await fetchStorageStatus();
      const usageText = document.getElementById('storage-usage-text');
      const barFill = document.getElementById('storage-bar-fill');
      const usedMb = (storage.database_size_bytes / (1024 * 1024)).toFixed(1);
      const capMb = (storage.disk_cap_bytes / (1024 * 1024)).toFixed(0);
      if (usageText) {
        usageText.textContent = `${usedMb} MB / ${capMb} MB (${storage.disk_usage_percent.toFixed(1)}%)`;
      }
      if (barFill) {
        barFill.style.width = `${Math.min(100, Math.max(1, storage.disk_usage_percent))}%`;
      }
    } catch (e) {
      console.warn('Storage status fetch failed:', e);
    }

    // 3. Alert Rules
    try {
      const rules = await fetchAlertRules();
      const tbody = document.getElementById('rules-tbody');
      if (tbody) {
        tbody.innerHTML = '';
        for (const rule of rules) {
          const tr = document.createElement('tr');
          const opSymbol = rule.operator === 'greater_than' ? '>' : '<';
          tr.innerHTML = `
            <td><strong>${rule.name}</strong></td>
            <td><span class="layer-code">${rule.layer}</span></td>
            <td>${rule.metric_name} ${opSymbol} ${rule.threshold}</td>
            <td>${rule.cooldown_seconds}s</td>
            <td><span class="pill-value">${rule.enabled ? 'ACTIVE' : 'DISABLED'}</span></td>
          `;
          tbody.appendChild(tr);
        }
      }
    } catch (e) {
      console.warn('Rules fetch failed:', e);
    }
  };

  btnSettings?.addEventListener('click', () => {
    if (modal) modal.style.display = 'flex';
    updateSettingsView();
  });

  btnCloseSettings?.addEventListener('click', () => {
    if (modal) modal.style.display = 'none';
  });

  // Password setup
  const btnSetPwd = document.getElementById('btn-set-password');
  const inputPwd = document.getElementById('input-master-pwd') as HTMLInputElement | null;
  btnSetPwd?.addEventListener('click', async () => {
    if (!inputPwd || !inputPwd.value.trim()) {
      alert('Please enter a master password.');
      return;
    }
    try {
      await setLanPassword(inputPwd.value.trim());
      alert('Master password configured successfully via Argon2id.');
      inputPwd.value = '';
      updateSettingsView();
    } catch (err) {
      alert('Failed to set password: ' + err);
    }
  });

  // LAN Toggle
  const toggleLan = document.getElementById('toggle-lan-mode') as HTMLInputElement | null;
  toggleLan?.addEventListener('change', async () => {
    try {
      await setLanMode(toggleLan.checked);
      updateSettingsView();
    } catch (err: any) {
      alert(err.message || 'Failed to toggle LAN mode');
      toggleLan.checked = !toggleLan.checked;
    }
  });

  // Discord Webhook
  const btnSaveWebhook = document.getElementById('btn-save-webhook');
  const btnTestDiscord = document.getElementById('btn-test-discord');
  const inputWebhook = document.getElementById('input-discord-webhook') as HTMLInputElement | null;

  btnSaveWebhook?.addEventListener('click', async () => {
    if (!inputWebhook || !inputWebhook.value.trim()) return;
    try {
      await setVaultSecret('DISCORD_WEBHOOK_URL', inputWebhook.value.trim());
      alert('Discord webhook URL encrypted and saved in Vault.');
      inputWebhook.value = '';
    } catch (err) {
      alert('Failed to save webhook: ' + err);
    }
  });

  btnTestDiscord?.addEventListener('click', async () => {
    const url = inputWebhook?.value.trim() || undefined;
    try {
      await testDiscordWebhook(url);
      alert('Discord rich embed alert dispatched successfully!');
    } catch (err: any) {
      alert(err.message || 'Discord webhook test failed');
    }
  });

  // Test Toast
  const btnTestToast = document.getElementById('btn-test-toast');
  btnTestToast?.addEventListener('click', async () => {
    try {
      await testToast('[HUMBOLDT] Notification Test', 'Win32 Toast notification subsystem operational.');
    } catch (err) {
      alert('Toast error: ' + err);
    }
  });

  // Storage Actions
  document.getElementById('btn-clear-cache')?.addEventListener('click', async () => {
    try {
      const res = await clearCaches();
      alert(`Cleared ${res.cleared_records} ephemeral records from cache.`);
      updateSettingsView();
    } catch (err) {
      alert('Clear caches error: ' + err);
    }
  });

  document.getElementById('btn-delete-snapshots')?.addEventListener('click', async () => {
    if (!confirm('Are you sure you want to delete ALL analytical snapshots?')) return;
    try {
      const res = await deleteAllSnapshots();
      alert(`Deleted ${res.deleted_snapshots} snapshots.`);
      updateSettingsView();
    } catch (err) {
      alert('Delete snapshots error: ' + err);
    }
  });

  document.getElementById('btn-compact-db')?.addEventListener('click', async () => {
    try {
      await compactStorage();
      alert('SQLite VACUUM complete. Free pages reclaimed.');
      updateSettingsView();
    } catch (err) {
      alert('Compaction error: ' + err);
    }
  });
}

/**
 * Updates notices in the Layer Rail for layers requiring upstream API keys.
 * If no key exists in the Vault, displays "no data available input an api key".
 */
async function refreshLayerRailKeyStatuses() {
  try {
    const keys = await fetchVaultKeys().catch(() => [] as string[]);
    const keyContainers = document.querySelectorAll<HTMLElement>('.layer-key-container');

    keyContainers.forEach((container) => {
      const providerId = container.getAttribute('data-provider-id');
      const layer = container.getAttribute('data-layer');
      if (!providerId) return;

      const secretName = `${providerId.toUpperCase()}_API_KEY`;
      const hasKey = keys.includes(secretName);

      if (hasKey) {
        container.innerHTML = `<span class="layer-notice-active" title="API key active in local Vault">✓ key saved</span>`;
      } else {
        container.innerHTML = `<span class="layer-notice-missing" title="Click to input API key for ${layer}">⚠ no data available input an api key</span>`;
        const noticeEl = container.querySelector('.layer-notice-missing');
        noticeEl?.addEventListener('click', (e) => {
          e.preventDefault();
          e.stopPropagation();
          openAttributionModal(providerId);
        });
      }
    });
  } catch (err) {
    console.warn('[HUMBOLDT] Failed to refresh layer rail key statuses:', err);
  }
}

let loadAttributionsGlobal: (() => Promise<void>) | null = null;

function openAttributionModal(providerId?: string) {
  const modal = document.getElementById('attribution-modal');
  if (modal) {
    modal.style.display = 'flex';
    if (loadAttributionsGlobal) {
      loadAttributionsGlobal().then(() => {
        if (providerId) {
          setTimeout(() => {
            const card = document.getElementById(`provider-card-${providerId}`);
            if (card) {
              card.scrollIntoView({ behavior: 'smooth', block: 'center' });
              card.classList.add('highlight-target');
              const input = card.querySelector<HTMLInputElement>('.provider-key-input');
              if (input && !input.disabled) input.focus();
              setTimeout(() => card.classList.remove('highlight-target'), 2500);
            }
          }, 150);
        }
      });
    }
  }
}

/**
 * Manages the data provider attribution modal, displaying open licenses and outbound links.
 */
function initAttributionModal() {
  const modal = document.getElementById('attribution-modal');
  const btnOpen = document.getElementById('btn-attribution');
  const btnClose = document.getElementById('btn-close-attribution');
  const gridEl = document.getElementById('attribution-grid');

  let loaded = false;

  const loadAttributions = async () => {
    if (loaded || !gridEl) return;
    try {
      const [providers, existingKeys] = await Promise.all([
        fetchAttributions(),
        fetchVaultKeys().catch(() => [] as string[]),
      ]);
      gridEl.innerHTML = '';

      // Group providers by category
      const categories: Record<string, ProviderAttribution[]> = {};
      for (const p of providers) {
        if (!categories[p.category]) categories[p.category] = [];
        categories[p.category].push(p);
      }

      for (const [cat, list] of Object.entries(categories)) {
        const catHeader = document.createElement('div');
        catHeader.className = 'attribution-category-title';
        catHeader.textContent = cat.toUpperCase();
        gridEl.appendChild(catHeader);

        const cardRow = document.createElement('div');
        cardRow.className = 'attribution-card-group';

        for (const prov of list) {
          const card = document.createElement('div');
          card.className = 'attribution-card';
          card.id = `provider-card-${prov.id}`;

          const titleRow = document.createElement('div');
          titleRow.className = 'attribution-card-header';

          const nameEl = document.createElement('span');
          nameEl.className = 'attribution-provider-name';
          nameEl.textContent = prov.name;
          titleRow.appendChild(nameEl);

          const badge = document.createElement('span');
          const badgeClass = prov.license_type ? prov.license_type.replace(/_/g, '-') : 'open-data';
          badge.className = `attribution-badge badge-${badgeClass}`;
          badge.textContent = prov.license;
          titleRow.appendChild(badge);

          const desc = document.createElement('p');
          desc.className = 'attribution-desc';
          desc.textContent = prov.description;

          const link = document.createElement('a');
          link.className = 'attribution-link';
          link.href = prov.url;
          link.target = '_blank';
          link.rel = 'noopener noreferrer';
          link.innerHTML = `Visit Provider Source ↗`;

          card.appendChild(titleRow);
          card.appendChild(desc);
          card.appendChild(link);

          // PROVIDER API KEY SECTION
          const keySection = document.createElement('div');
          keySection.className = 'provider-key-section';

          const keyLabel = document.createElement('span');
          keyLabel.className = 'provider-key-label';
          keyLabel.textContent = 'Provider API Key / Token:';

          const keyRow = document.createElement('div');
          keyRow.className = 'provider-key-row';

          const keyInput = document.createElement('input');
          keyInput.className = 'provider-key-input';

          const btnTest = document.createElement('button');
          btnTest.className = 'btn btn-secondary btn-test-key';
          btnTest.textContent = 'Test Key';

          const btnChange = document.createElement('button');
          btnChange.className = 'btn btn-secondary btn-change-key';
          btnChange.textContent = 'Change Key';

          const statusMsg = document.createElement('span');
          statusMsg.className = 'key-status-msg';

          // Check if key already saved in vault
          const vaultKeyName = `${prov.id.toUpperCase()}_API_KEY`;
          const isSaved = existingKeys.includes(vaultKeyName);

          if (isSaved) {
            keyInput.type = 'text';
            keyInput.value = 'key saved';
            keyInput.disabled = true;
            keyInput.classList.add('saved');
            btnTest.style.display = 'none';
            btnChange.style.display = 'inline-block';
            statusMsg.textContent = 'key valid';
            statusMsg.className = 'key-status-msg text-success';
          } else {
            keyInput.type = 'password';
            keyInput.placeholder = 'Enter API key or token...';
            keyInput.disabled = false;
            btnTest.style.display = 'inline-block';
            btnChange.style.display = 'none';
          }

          btnTest.addEventListener('click', async () => {
            const keyVal = keyInput.value.trim();
            if (!keyVal) {
              statusMsg.textContent = 'invalid key';
              statusMsg.className = 'key-status-msg text-danger';
              return;
            }
            statusMsg.textContent = 'testing...';
            statusMsg.className = 'key-status-msg';

            try {
              const res = await testProviderKey(prov.id, keyVal);
              if (res.valid) {
                statusMsg.textContent = 'key valid';
                statusMsg.className = 'key-status-msg text-success';
                keyInput.type = 'text';
                keyInput.value = 'key saved';
                keyInput.disabled = true;
                keyInput.classList.add('saved');
                btnTest.style.display = 'none';
                btnChange.style.display = 'inline-block';
                await refreshLayerRailKeyStatuses();
              } else {
                // If test fails: do NOT change the box the key was entered in and instead just say "invalid key"
                statusMsg.textContent = 'invalid key';
                statusMsg.className = 'key-status-msg text-danger';
              }
            } catch {
              statusMsg.textContent = 'invalid key';
              statusMsg.className = 'key-status-msg text-danger';
            }
          });

          btnChange.addEventListener('click', async () => {
            try {
              await deleteProviderKey(prov.id);
              await refreshLayerRailKeyStatuses();
            } catch (e) {
              console.warn('Failed to delete key from vault:', e);
            }
            // Resets it to the no key state and you have to input one again incase of expired keys
            keyInput.type = 'password';
            keyInput.value = '';
            keyInput.disabled = false;
            keyInput.classList.remove('saved');
            keyInput.placeholder = 'Enter API key or token...';
            statusMsg.textContent = '';
            btnChange.style.display = 'none';
            btnTest.style.display = 'inline-block';
            keyInput.focus();
          });

          keyRow.appendChild(keyInput);
          keyRow.appendChild(btnTest);
          keyRow.appendChild(btnChange);
          keyRow.appendChild(statusMsg);

          keySection.appendChild(keyLabel);
          keySection.appendChild(keyRow);
          card.appendChild(keySection);

          cardRow.appendChild(card);
        }

        gridEl.appendChild(cardRow);
      }

      loaded = true;
    } catch (err) {
      if (gridEl) {
        gridEl.innerHTML = `<div class="alert-box alert-warning">Failed to load provider attributions: ${err}</div>`;
      }
    }
  };

  loadAttributionsGlobal = loadAttributions;

  btnOpen?.addEventListener('click', () => {
    if (modal) {
      modal.style.display = 'flex';
      loadAttributions();
    }
  });

  btnClose?.addEventListener('click', () => {
    if (modal) modal.style.display = 'none';
  });

  modal?.addEventListener('click', (e) => {
    if (e.target === modal) modal.style.display = 'none';
  });
}

document.addEventListener('DOMContentLoaded', bootstrap);
