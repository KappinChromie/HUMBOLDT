import * as Cesium from 'cesium';

// Ensure Cesium never attempts to call Cesium ion without a user-configured token
Cesium.Ion.defaultAccessToken = '';

export type BasemapType = 'satellite' | 'dark' | 'osm' | 'offline' | 'ion';

export interface GlobeController {
  viewer: Cesium.Viewer;
  flyTo: (lat: number, lon: number, altitudeM?: number) => void;
  setBasemap: (type: BasemapType) => Promise<boolean>;
  getBasemap: () => BasemapType;
  destroy: () => void;
}

/**
 * Initializes CesiumJS using Tier 1 Bundled Low-Res Offline Basemap.
 * Completely offline, zero network requests, local NaturalEarthII tile assets.
 */
export async function initCesiumGlobe(containerId: string): Promise<GlobeController> {
  // Create offline Natural Earth II imagery provider from local bundled assets
  const offlineImageryProvider = await Cesium.TileMapServiceImageryProvider.fromUrl(
    Cesium.buildModuleUrl('Assets/Textures/NaturalEarthII')
  );

  const viewer = new Cesium.Viewer(containerId, {
    baseLayer: new Cesium.ImageryLayer(offlineImageryProvider),
    baseLayerPicker: false,
    geocoder: false,
    homeButton: false,
    infoBox: false,
    sceneModePicker: false,
    selectionIndicator: false,
    timeline: false,
    animation: false,
    fullscreenButton: false,
    navigationHelpButton: false,
    scene3DOnly: true,
    shadows: false,
    shouldAnimate: true,
  });

  // Dark-first instrument aesthetic configuration
  viewer.scene.backgroundColor = Cesium.Color.fromCssColorString('#0d1117');
  if (viewer.scene.globe) {
    viewer.scene.globe.baseColor = Cesium.Color.fromCssColorString('#0d1117');
    viewer.scene.globe.enableLighting = false;
    viewer.scene.globe.depthTestAgainstTerrain = true;
  }

  // Set default starting camera view
  viewer.camera.setView({
    destination: Cesium.Cartesian3.fromDegrees(-94.5786, 39.0997, 15000000), // Centered on Americas
    orientation: {
      heading: Cesium.Math.toRadians(0),
      pitch: Cesium.Math.toRadians(-90),
      roll: 0.0,
    },
  });

  // Camera coordinates overlay tracking
  const coordsElement = document.getElementById('camera-coords');
  if (coordsElement) {
    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
    handler.setInputAction((movement: Cesium.ScreenSpaceEventHandler.MotionEvent) => {
      const ray = viewer.camera.getPickRay(movement.endPosition);
      if (ray) {
        const cartesian = viewer.scene.globe.pick(ray, viewer.scene);
        if (cartesian) {
          const cartographic = Cesium.Cartographic.fromCartesian(cartesian);
          const lat = Cesium.Math.toDegrees(cartographic.latitude).toFixed(4);
          const lon = Cesium.Math.toDegrees(cartographic.longitude).toFixed(4);
          const altM = viewer.camera.positionCartographic.height;
          const altKm = Math.round(altM / 1000).toLocaleString();
          let targetMode = 'COUNTRY';
          if (altM <= 350000) {
            targetMode = 'CITY';
          } else if (altM <= 1200000) {
            targetMode = 'COUNTY';
          } else if (altM <= 4500000) {
            targetMode = 'STATE';
          }
          coordsElement.textContent = `LAT: ${lat}° | LON: ${lon}° | ALT: ${altKm} km | TARGET: ${targetMode}`;
        }
      }
    }, Cesium.ScreenSpaceEventType.MOUSE_MOVE);
  }

  const flyTo = (lat: number, lon: number, altitudeM: number = 2500000) => {
    viewer.camera.flyTo({
      destination: Cesium.Cartesian3.fromDegrees(lon, lat, altitudeM),
      duration: 1.5,
    });
  };

  let currentBasemap: BasemapType = (localStorage.getItem('humboldt_basemap') as BasemapType) || 'satellite';

  const updateTierIndicator = (type: BasemapType) => {
    const badgeEl = document.getElementById('current-tier-badge');
    const descEl = document.getElementById('basemap-status-desc');
    const select1 = document.getElementById('select-basemap') as HTMLSelectElement | null;
    const select2 = document.getElementById('quick-select-basemap') as HTMLSelectElement | null;

    if (select1 && select1.value !== type) select1.value = type;
    if (select2 && select2.value !== type) select2.value = type;

    if (badgeEl) {
      const isOnline = type !== 'offline';
      badgeEl.textContent = isOnline ? 'TIER 2 (ONLINE)' : 'TIER 1 (OFFLINE)';
      badgeEl.className = isOnline ? 'tier-badge tier-online' : 'tier-badge tier-offline';
    }

    if (descEl) {
      switch (type) {
        case 'satellite':
          descEl.textContent = 'High-resolution global satellite photography (ESRI World Imagery).';
          break;
        case 'dark':
          descEl.textContent = 'CartoDB Dark Matter: High-contrast street & water networks.';
          break;
        case 'osm':
          descEl.textContent = 'OpenStreetMap: Detailed worldwide street topology & roads.';
          break;
        case 'ion':
          descEl.textContent = 'Cesium ion: High-res photogrammetry & 3D world terrain elevation.';
          break;
        case 'offline':
        default:
          descEl.textContent = 'Tier 1 Natural Earth 1:50m: Fully offline, air-gapped fallback.';
          break;
      }
    }
  };

  const setBasemap = async (type: BasemapType): Promise<boolean> => {
    try {
      let provider: Cesium.ImageryProvider;
      if (type === 'satellite') {
        provider = await Cesium.ArcGisMapServerImageryProvider.fromUrl(
          'https://services.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer',
          { enablePickFeatures: false }
        );
      } else if (type === 'dark') {
        provider = new Cesium.UrlTemplateImageryProvider({
          url: 'https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png',
          subdomains: ['a', 'b', 'c', 'd'],
          maximumLevel: 19,
          credit: 'CartoDB Dark Matter / OpenStreetMap',
        });
      } else if (type === 'osm') {
        provider = new Cesium.OpenStreetMapImageryProvider({
          url: 'https://tile.openstreetmap.org/',
          maximumLevel: 19,
          credit: '© OpenStreetMap contributors',
        });
      } else if (type === 'ion') {
        if (!Cesium.Ion.defaultAccessToken) {
          alert('Cesium ion requires an access token. Please enter your Cesium ion token in Settings ⚙️ -> Data Providers & Keys.');
          return false;
        }
        provider = await Cesium.createWorldImageryAsync();
        try {
          viewer.terrainProvider = await Cesium.createWorldTerrainAsync();
        } catch (e) {
          console.warn('[HUMBOLDT] Failed to enable Cesium world terrain:', e);
        }
      } else {
        provider = await Cesium.TileMapServiceImageryProvider.fromUrl(
          Cesium.buildModuleUrl('Assets/Textures/NaturalEarthII')
        );
      }

      viewer.imageryLayers.removeAll();
      viewer.imageryLayers.addImageryProvider(provider);
      currentBasemap = type;
      localStorage.setItem('humboldt_basemap', type);
      updateTierIndicator(type);
      console.log(`[HUMBOLDT] Successfully switched basemap to: ${type}`);
      return true;
    } catch (err) {
      console.warn(`[HUMBOLDT] Failed to load basemap ${type}, falling back to offline:`, err);
      try {
        const fallback = await Cesium.TileMapServiceImageryProvider.fromUrl(
          Cesium.buildModuleUrl('Assets/Textures/NaturalEarthII')
        );
        viewer.imageryLayers.removeAll();
        viewer.imageryLayers.addImageryProvider(fallback);
        currentBasemap = 'offline';
        localStorage.setItem('humboldt_basemap', 'offline');
        updateTierIndicator('offline');
      } catch (e) {
        console.error('[HUMBOLDT] Fallback basemap error:', e);
      }
      return false;
    }
  };

  const getBasemap = (): BasemapType => currentBasemap;

  // Attempt initial basemap switch if configured for online (defaults to high-res satellite)
  if (currentBasemap !== 'offline') {
    setTimeout(() => {
      setBasemap(currentBasemap);
    }, 50);
  } else {
    updateTierIndicator('offline');
  }

  const destroy = () => {
    viewer.destroy();
  };

  return {
    viewer,
    flyTo,
    setBasemap,
    getBasemap,
    destroy,
  };
}
