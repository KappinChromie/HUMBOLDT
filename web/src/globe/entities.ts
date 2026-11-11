import * as Cesium from 'cesium';
import {
  Observation,
  resolveGeo,
  fetchNews,
  NewsArticle,
  ResolvedHierarchy,
  BoundariesResponse,
  GeoPolygon,
  SubmarineCable,
} from '../services/api';

function getCompassHeading(deg: number): string {
  const directions = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];
  const index = Math.round(((deg % 360) / 45)) % 8;
  return directions[index];
}

function getAircraftSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M21 16v-2l-8-5V3.5c0-.83-.67-1.5-1.5-1.5S10 2.67 10 3.5V9l-8 5v2l8-2.5V19l-2 1.5V22l3.5-1 3.5 1v-1.5L13 19v-5.5l8 2.5z" fill="${color}" transform="scale(0.7) translate(5.2, 5.2)"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getShipSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M6 15l2 3h8l2-3-1-3H7l-1 3zm6-9v6m-3-3h6" stroke="${color}" stroke-width="1.8" stroke-linecap="round" fill="none"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getSatelliteSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <rect x="9" y="9" width="6" height="6" fill="${color}" rx="1"/>
    <rect x="2" y="10" width="5" height="4" fill="${color}" opacity="0.85"/>
    <rect x="17" y="10" width="5" height="4" fill="${color}" opacity="0.85"/>
    <line x1="7" y1="12" x2="9" y2="12" stroke="${color}" stroke-width="1.5"/>
    <line x1="15" y1="12" x2="17" y2="12" stroke="${color}" stroke-width="1.5"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getQuakeSvg(color: string, size: number): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="2"/>
    <path d="M4 12h3l2-5 3 10 2.5-7 2.5 4h3" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getWeatherSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M12 7a4 4 0 0 1 3.9 3.1 3.5 3.5 0 0 1 2.1 3.4 3.5 3.5 0 0 1-3.5 3.5H8.5A3.5 3.5 0 0 1 5 13.5a3.5 3.5 0 0 1 2.5-3.4A4 4 0 0 1 12 7z" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getWildfireSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M12 3c1.5 3 4 5 4 8a4 4 0 0 1-8 0c0-3 2.5-5 4-8z" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getAirQualitySvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="4" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getHydroSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M12 4s-4 5.5-4 8a4 4 0 0 0 8 0c0-2.5-4-8-4-8z" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getBuoySvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M12 7v10M8 12h8" stroke="${color}" stroke-width="2" stroke-linecap="round"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getCycloneSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="30" height="30" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.8"/>
    <path d="M12 6c3 0 5 2 5 5 0 2-1.5 3.5-3.5 3.5S10 13 10 11.5c0-1 1-1.5 2-1.5M12 18c-3 0-5-2-5-5 0-2 1.5-3.5 3.5-3.5S14 11 14 12.5c0 1-1 1.5-2 1.5" fill="none" stroke="${color}" stroke-width="2" stroke-linecap="round"/>
    <circle cx="12" cy="12" r="1.5" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getVolcanoSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <path d="M4 19l6-11h4l6 11H4z" fill="${color}" fill-opacity="0.8"/>
    <path d="M10 8l-1-3M12 8V4M14 8l1-3" stroke="${color}" stroke-width="1.8" stroke-linecap="round"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getCableLandingSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="10" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="4.5" fill="${color}"/>
    <circle cx="12" cy="12" r="7.5" fill="none" stroke="${color}" stroke-width="1" stroke-dasharray="2 2"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getRadiationSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <circle cx="12" cy="12" r="11" fill="#161b22" stroke="${color}" stroke-width="1.5"/>
    <circle cx="12" cy="12" r="2.2" fill="${color}"/>
    <path d="M12 9.5l2.5-4.3a6 6 0 0 0-5 0L12 9.5zM9.8 13.2l-4.3 2.5a6 6 0 0 0 2.5 4.3l1.8-4.8v-2zM14.2 13.2l4.3 2.5a6 6 0 0 1-2.5 4.3l-1.8-4.8v-2z" fill="${color}"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

function getDisasterSvg(color: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24">
    <path d="M12 2l10 18H2L12 2z" fill="#161b22" stroke="${color}" stroke-width="1.8" stroke-linejoin="round"/>
    <path d="M12 8v6M12 17v1.5" stroke="${color}" stroke-width="2" stroke-linecap="round"/>
  </svg>`;
  return 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(svg);
}

export type GeoHierarchyLevel = 'country' | 'state' | 'county' | 'city';

export function getHierarchyLevelForAltitude(altitudeM: number): GeoHierarchyLevel {
  if (altitudeM > 4_500_000) {
    return 'country';
  } else if (altitudeM > 1_200_000) {
    return 'state';
  } else if (altitudeM > 350_000) {
    return 'county';
  } else {
    return 'city';
  }
}

interface MovingKinematicEntity {
  id: string;
  layer: 'aircraft' | 'ship' | 'satellite';
  lat: number;
  lon: number;
  altM: number;
  speedKts: number;
  headingDeg: number;
  cesiumEntity: Cesium.Entity;
}

export class EntityLayerManager {
  private viewer: Cesium.Viewer;
  private aircraftDataSource: Cesium.CustomDataSource;
  private shipDataSource: Cesium.CustomDataSource;
  private satDataSource: Cesium.CustomDataSource;
  private quakeDataSource: Cesium.CustomDataSource;
  private weatherDataSource: Cesium.CustomDataSource;
  private selectionDataSource: Cesium.CustomDataSource;
  private reticleDataSource: Cesium.CustomDataSource;
  private countryBoundariesDataSource: Cesium.CustomDataSource;
  private stateBoundariesDataSource: Cesium.CustomDataSource;
  private countyBoundariesDataSource: Cesium.CustomDataSource;
  private cityBoundariesDataSource: Cesium.CustomDataSource;
  private placesDataSource: Cesium.CustomDataSource;
  private wildfireDataSource: Cesium.CustomDataSource;
  private airQualityDataSource: Cesium.CustomDataSource;
  private hydroDataSource: Cesium.CustomDataSource;
  private buoyDataSource: Cesium.CustomDataSource;
  private cycloneDataSource: Cesium.CustomDataSource;
  private volcanoDataSource: Cesium.CustomDataSource;
  private cableDataSource: Cesium.CustomDataSource;
  private radiationDataSource: Cesium.CustomDataSource;
  private auroraDataSource: Cesium.CustomDataSource;
  private disasterDataSource: Cesium.CustomDataSource;
  private boundaries: BoundariesResponse | null = null;
  private movingEntities: Map<string, MovingKinematicEntity> = new Map();
  private isSimulationPaused: boolean = false;
  private simulationSpeed: number = 1.0;
  private lastFrameTime: number = performance.now();
  private selectedEntityId: string | null = null;

  constructor(viewer: Cesium.Viewer) {
    this.viewer = viewer;
    this.countryBoundariesDataSource = new Cesium.CustomDataSource('boundaries_countries');
    this.stateBoundariesDataSource = new Cesium.CustomDataSource('boundaries_states');
    this.countyBoundariesDataSource = new Cesium.CustomDataSource('boundaries_counties');
    this.cityBoundariesDataSource = new Cesium.CustomDataSource('boundaries_cities');
    this.placesDataSource = new Cesium.CustomDataSource('boundaries_places');
    this.aircraftDataSource = new Cesium.CustomDataSource('aircraft');
    this.shipDataSource = new Cesium.CustomDataSource('ships');
    this.satDataSource = new Cesium.CustomDataSource('satellites');
    this.quakeDataSource = new Cesium.CustomDataSource('earthquakes');
    this.weatherDataSource = new Cesium.CustomDataSource('weather');
    this.wildfireDataSource = new Cesium.CustomDataSource('wildfires');
    this.airQualityDataSource = new Cesium.CustomDataSource('air_quality');
    this.hydroDataSource = new Cesium.CustomDataSource('hydro');
    this.buoyDataSource = new Cesium.CustomDataSource('buoys');
    this.cycloneDataSource = new Cesium.CustomDataSource('cyclones');
    this.volcanoDataSource = new Cesium.CustomDataSource('volcanoes');
    this.cableDataSource = new Cesium.CustomDataSource('cables');
    this.radiationDataSource = new Cesium.CustomDataSource('radiation');
    this.auroraDataSource = new Cesium.CustomDataSource('aurora');
    this.disasterDataSource = new Cesium.CustomDataSource('disasters');
    this.selectionDataSource = new Cesium.CustomDataSource('selection');
    this.reticleDataSource = new Cesium.CustomDataSource('reticle');

    // Bottom layer: Country/State/County/City boundary lines and selection fills
    this.viewer.dataSources.add(this.countryBoundariesDataSource);
    this.viewer.dataSources.add(this.stateBoundariesDataSource);
    this.viewer.dataSources.add(this.countyBoundariesDataSource);
    this.viewer.dataSources.add(this.cityBoundariesDataSource);
    this.viewer.dataSources.add(this.selectionDataSource);

    // Middle layers: Places and Observation entities (air, maritime, space, disasters, sensors)
    this.viewer.dataSources.add(this.placesDataSource);
    this.viewer.dataSources.add(this.aircraftDataSource);
    this.viewer.dataSources.add(this.shipDataSource);
    this.viewer.dataSources.add(this.satDataSource);
    this.viewer.dataSources.add(this.quakeDataSource);
    this.viewer.dataSources.add(this.weatherDataSource);
    this.viewer.dataSources.add(this.wildfireDataSource);
    this.viewer.dataSources.add(this.airQualityDataSource);
    this.viewer.dataSources.add(this.hydroDataSource);
    this.viewer.dataSources.add(this.buoyDataSource);
    this.viewer.dataSources.add(this.cycloneDataSource);
    this.viewer.dataSources.add(this.volcanoDataSource);
    this.viewer.dataSources.add(this.cableDataSource);
    this.viewer.dataSources.add(this.radiationDataSource);
    this.viewer.dataSources.add(this.auroraDataSource);
    this.viewer.dataSources.add(this.disasterDataSource);

    // Topmost layer: Selection reticle & pinpoint coordinate marker
    this.viewer.dataSources.add(this.reticleDataSource);

    this.setupEntityPicking();
    this.setupKinematicAnimationLoop();
  }

  /**
   * Continuous 60 FPS kinematic simulation loop.
   * Dead-reckons moving entities along great circle paths based on real-world velocity vectors.
   */
  private setupKinematicAnimationLoop() {
    this.viewer.scene.preUpdate.addEventListener(() => {
      const now = performance.now();
      const rawDt = (now - this.lastFrameTime) / 1000.0;
      this.lastFrameTime = now;

      // Update boundary outlines and place labels visibility according to camera altitude
      const cartographic = this.viewer.camera.positionCartographic;
      const cameraAltitudeM = cartographic ? cartographic.height : 10_000_000;

      // Google Maps style hierarchical visibility:
      // - Really zoomed out (> 5,500 km): country outlines only.
      // - Zoom in (<= 5,500 km): state outlines and major cities appear.
      // - Zoom in more (<= 1,500 km): county outlines and metro cities appear.
      // - Zoom in close (<= 450 km): municipal city boundaries appear.
      this.countryBoundariesDataSource.show = true;
      this.stateBoundariesDataSource.show = cameraAltitudeM <= 5_500_000;
      this.countyBoundariesDataSource.show = cameraAltitudeM <= 1_500_000;
      this.cityBoundariesDataSource.show = cameraAltitudeM <= 450_000;
      this.placesDataSource.show = cameraAltitudeM <= 5_500_000;

      if (this.isSimulationPaused) return;

      // Clamp delta time to avoid large jumps if tab was backgrounded (max 0.25s)
      const dt = Math.min(rawDt, 0.25) * this.simulationSpeed;
      if (dt <= 0) return;

      for (const entity of this.movingEntities.values()) {
        const earthRadiusM = entity.layer === 'satellite' ? 6371000 + entity.altM : 6371000;
        const speedMps = entity.speedKts * 0.514444;
        const distanceM = speedMps * dt;
        const delta = distanceM / earthRadiusM;
        const theta = (entity.headingDeg * Math.PI) / 180;
        const phi1 = (entity.lat * Math.PI) / 180;
        const lambda1 = (entity.lon * Math.PI) / 180;

        const phi2 = Math.asin(
          Math.sin(phi1) * Math.cos(delta) + Math.cos(phi1) * Math.sin(delta) * Math.cos(theta)
        );
        const lambda2 = lambda1 + Math.atan2(
          Math.sin(theta) * Math.sin(delta) * Math.cos(phi1),
          Math.cos(delta) - Math.sin(phi1) * Math.sin(phi2)
        );

        let newLat = (phi2 * 180) / Math.PI;
        let newLon = (lambda2 * 180) / Math.PI;

        while (newLon > 180) newLon -= 360;
        while (newLon < -180) newLon += 360;

        entity.lat = newLat;
        entity.lon = newLon;

        const newPos = Cesium.Cartesian3.fromDegrees(newLon, newLat, entity.altM);
        entity.cesiumEntity.position = new Cesium.ConstantPositionProperty(newPos);

        // If currently selected in Context Panel, live update coordinate tile!
        if (this.selectedEntityId === entity.id) {
          const grid = document.getElementById('entity-breakdown-grid');
          if (grid) {
            const tiles = grid.querySelectorAll('.breakdown-tile');
            for (let i = 0; i < tiles.length; i++) {
              const labelEl = tiles[i].querySelector('.breakdown-tile-label');
              const valEl = tiles[i].querySelector('.breakdown-tile-value');
              if (labelEl && valEl) {
                const labelText = labelEl.textContent || '';
                if (labelText.includes('Coordinates') || labelText.includes('Sub-Satellite Point')) {
                  valEl.textContent = `${newLat.toFixed(4)}°, ${newLon.toFixed(4)}°`;
                }
              }
            }
          }
        }
      }
    });
  }

  public setSimulationPaused(paused: boolean) {
    this.isSimulationPaused = paused;
  }

  public isPaused(): boolean {
    return this.isSimulationPaused;
  }

  public setSimulationSpeed(multiplier: number) {
    this.simulationSpeed = multiplier;
  }

  public getSimulationSpeed(): number {
    return this.simulationSpeed;
  }

  /**
   * Dispatches a single live observation to the corresponding datasource.
   */
  public handleObservation(obs: Observation) {
    switch (obs.layer) {
      case 'aircraft':
        this.upsertAircraft(obs);
        break;
      case 'ship':
        this.upsertShip(obs);
        break;
      case 'satellite':
        this.upsertSatellite(obs);
        break;
      case 'quake':
        this.upsertQuake(obs);
        break;
      case 'weather':
        this.upsertWeather(obs);
        break;
      case 'wildfire':
        this.upsertWildfire(obs);
        break;
      case 'air_quality':
        this.upsertAirQuality(obs);
        break;
      case 'hydro':
        this.upsertHydro(obs);
        break;
      case 'buoy':
        this.upsertBuoy(obs);
        break;
      case 'cyclone':
        this.upsertCyclone(obs);
        break;
      case 'volcano':
        this.upsertVolcano(obs);
        break;
      case 'radiation':
        this.upsertRadiation(obs);
        break;
      case 'disaster':
        this.upsertDisaster(obs);
        break;
      case 'space_weather':
        this.updateSpaceWeatherDisplay(obs);
        break;
      case 'power_grid':
        this.updatePowerGridDisplay(obs);
        break;
    }
  }

  /**
   * Updates or inserts an aircraft entity on the 3D globe.
   */
  public upsertAircraft(ac: Observation) {
    if (ac.latitude === undefined || ac.longitude === undefined) return;

    const id = `aircraft_${ac.entity_id ?? Math.random()}`;
    const altM = ac.altitude_m ?? 10000;
    const callsign = ac.attributes?.flight?.trim() || ac.entity_id || 'AIRCRAFT';
    const track = ac.attributes?.track ?? ac.attributes?.track_deg ?? 0;
    const gs = ac.attributes?.gs !== undefined 
      ? Math.round(ac.attributes.gs) 
      : (ac.attributes?.ground_speed_knots !== undefined ? Math.round(ac.attributes.ground_speed_knots) : 450);
    const squawk = ac.attributes?.squawk ?? '----';

    const position = Cesium.Cartesian3.fromDegrees(ac.longitude, ac.latitude, altM);

    // Altitude color ramp: low (#3fb950 green), medium (#58a6ff cyan), cruise/high (#d2a8ff purple)
    let colorHex = '#58a6ff';
    if (altM < 3000) {
      colorHex = '#3fb950';
    } else if (altM > 10000) {
      colorHex = '#d2a8ff';
    }

    const existing = this.aircraftDataSource.entities.getById(id);
    let entity: Cesium.Entity;
    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
      if (existing.billboard) {
        existing.billboard.rotation = new Cesium.ConstantProperty(Cesium.Math.toRadians(track));
      }
      if (existing.properties) {
        existing.properties.observation = new Cesium.ConstantProperty(ac);
        existing.properties.subtitle = new Cesium.ConstantProperty(
          `Alt: ${Math.round(altM).toLocaleString()} m | Speed: ${gs ?? '—'} kts | Heading: ${track}°`
        );
      }
      entity = existing;
    } else {
      entity = this.aircraftDataSource.entities.add({
        id,
        name: `Flight ${callsign}`,
        position,
        billboard: {
          image: getAircraftSvg(colorHex),
          width: 22,
          height: 22,
          rotation: Cesium.Math.toRadians(track),
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -30),
        },
        label: {
          text: ` ${callsign}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString('#c9d1d9'),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, 800000),
          eyeOffset: new Cesium.Cartesian3(0, 0, -30),
        },
        properties: new Cesium.PropertyBag({
          layer: 'aircraft',
          observation: ac,
          title: `Flight: ${callsign} (${ac.entity_id})`,
          subtitle: `Alt: ${Math.round(altM).toLocaleString()} m | Speed: ${gs ?? '—'} kts | Heading: ${track}° | Squawk: ${squawk}`,
          provider: ac.provider,
        }),
      });
    }

    this.movingEntities.set(id, {
      id,
      layer: 'aircraft',
      lat: ac.latitude,
      lon: ac.longitude,
      altM,
      speedKts: gs,
      headingDeg: track,
      cesiumEntity: entity,
    });
  }

  public renderAircraft(aircraftList: Observation[]) {
    this.aircraftDataSource.entities.removeAll();
    for (const [key, ent] of this.movingEntities.entries()) {
      if (ent.layer === 'aircraft') {
        this.movingEntities.delete(key);
      }
    }
    for (const ac of aircraftList) {
      this.upsertAircraft(ac);
    }
  }

  /**
   * Updates or inserts a vessel (AIS) entity on the 3D globe.
   */
  public upsertShip(ship: Observation) {
    if (ship.latitude === undefined || ship.longitude === undefined) return;

    const id = `ship_${ship.entity_id ?? Math.random()}`;
    const name = ship.attributes?.name?.trim() || `MMSI-${ship.entity_id}`;
    const sog = ship.attributes?.sog !== undefined 
      ? Number(ship.attributes.sog) 
      : (ship.attributes?.speed !== undefined ? Number(ship.attributes.speed) : 14.0);
    const cog = ship.attributes?.cog !== undefined 
      ? Number(ship.attributes.cog) 
      : (ship.attributes?.heading !== undefined ? Number(ship.attributes.heading) : 90.0);
    const position = Cesium.Cartesian3.fromDegrees(ship.longitude, ship.latitude, 100);

    const existing = this.shipDataSource.entities.getById(id);
    let entity: Cesium.Entity;
    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
      entity = existing;
    } else {
      entity = this.shipDataSource.entities.add({
        id,
        name: `Vessel ${name}`,
        position,
        billboard: {
          image: getShipSvg('#388bfd'),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -40),
        },
        label: {
          text: ` ${name}`,
          font: '9px monospace',
          fillColor: Cesium.Color.fromCssColorString('#8b949e'),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, 500000),
          eyeOffset: new Cesium.Cartesian3(0, 0, -40),
        },
        properties: new Cesium.PropertyBag({
          layer: 'ship',
          observation: ship,
          title: `Vessel: ${name}`,
          subtitle: `MMSI: ${ship.entity_id} | Provider: ${ship.provider}`,
          provider: ship.provider,
        }),
      });
    }

    this.movingEntities.set(id, {
      id,
      layer: 'ship',
      lat: ship.latitude,
      lon: ship.longitude,
      altM: 100,
      speedKts: sog,
      headingDeg: cog,
      cesiumEntity: entity,
    });
  }

  public renderShips(ships: Observation[]) {
    this.shipDataSource.entities.removeAll();
    for (const [key, ent] of this.movingEntities.entries()) {
      if (ent.layer === 'ship') {
        this.movingEntities.delete(key);
      }
    }
    for (const s of ships) {
      this.upsertShip(s);
    }
  }

  /**
   * Updates or inserts a satellite entity on the 3D globe.
   */
  public upsertSatellite(sat: Observation) {
    if (sat.latitude === undefined || sat.longitude === undefined) return;

    const id = `sat_${sat.entity_id ?? Math.random()}`;
    const altM = sat.altitude_m ?? 400000;
    const satName = sat.attributes?.name ?? `SAT-${sat.entity_id}`;
    const position = Cesium.Cartesian3.fromDegrees(sat.longitude, sat.latitude, altM);

    const existing = this.satDataSource.entities.getById(id);
    let entity: Cesium.Entity;
    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
      entity = existing;
    } else {
      entity = this.satDataSource.entities.add({
        id,
        name: satName,
        position,
        billboard: {
          image: getSatelliteSvg('#58a6ff'),
          width: 22,
          height: 22,
          verticalOrigin: Cesium.VerticalOrigin.CENTER,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
        },
        label: {
          text: ` ${satName}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString('#c9d1d9'),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -4),
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, 35000000),
        },
        properties: new Cesium.PropertyBag({
          layer: 'satellite',
          observation: sat,
          title: `Satellite: ${satName}`,
          subtitle: `NORAD ID: ${sat.entity_id} | Altitude: ${Math.round(altM / 1000).toLocaleString()} km`,
          provider: sat.provider,
        }),
      });
    }

    this.movingEntities.set(id, {
      id,
      layer: 'satellite',
      lat: sat.latitude,
      lon: sat.longitude,
      altM,
      speedKts: 14890,
      headingDeg: sat.attributes?.inclination_deg ?? 51.6,
      cesiumEntity: entity,
    });
  }

  public renderSatellites(satellites: Observation[]) {
    this.satDataSource.entities.removeAll();
    for (const [key, ent] of this.movingEntities.entries()) {
      if (ent.layer === 'satellite') {
        this.movingEntities.delete(key);
      }
    }
    for (const sat of satellites) {
      this.upsertSatellite(sat);
    }
  }

  /**
   * Updates or inserts earthquake markers on the 3D globe.
   */
  public upsertQuake(q: Observation) {
    if (q.latitude === undefined || q.longitude === undefined) return;

    const id = `quake_${q.entity_id ?? Math.random()}`;
    const mag = q.metric_value ?? 0;
    const depthKm = q.altitude_m !== undefined ? Math.abs(q.altitude_m / 1000).toFixed(1) : '0';
    const place = q.attributes?.place ?? 'Unknown Region';

    let colorHex = '#d29922'; // Amber (<4.5)
    if (mag >= 6.0) {
      colorHex = '#f85149'; // Alert red (>=6)
    } else if (mag >= 4.5) {
      colorHex = '#f0883e'; // Orange (4.5 - 6)
    }

    const iconSize = Math.max(18, Math.min(34, Math.round(mag * 4.8)));

    const existing = this.quakeDataSource.entities.getById(id);
    if (!existing) {
      this.quakeDataSource.entities.add({
        id,
        name: `Earthquake M ${mag.toFixed(1)}`,
        position: Cesium.Cartesian3.fromDegrees(q.longitude, q.latitude, 150),
        billboard: {
          image: getQuakeSvg(colorHex, iconSize),
          width: iconSize,
          height: iconSize,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'quake',
          observation: q,
          title: `M ${mag.toFixed(1)} - ${place}`,
          subtitle: `Depth: ${depthKm} km | WGS84: [${q.latitude.toFixed(3)}°, ${q.longitude.toFixed(3)}°]`,
          provider: q.provider,
        }),
      });
    }
  }

  public renderQuakes(quakes: Observation[]) {
    this.quakeDataSource.entities.removeAll();
    for (const q of quakes) {
      this.upsertQuake(q);
    }
  }

  /**
   * Updates weather station observation markers.
   */
  public upsertWeather(w: Observation) {
    if (w.latitude === undefined || w.longitude === undefined) return;

    const id = `weather_${w.entity_id ?? Math.random()}`;
    const tempC = w.metric_value !== undefined ? `${w.metric_value.toFixed(1)}°C` : '—';
    const station = w.entity_id ?? 'Weather Station';

    const existing = this.weatherDataSource.entities.getById(id);
    if (!existing) {
      this.weatherDataSource.entities.add({
        id,
        name: `Weather: ${station}`,
        position: Cesium.Cartesian3.fromDegrees(w.longitude, w.latitude, 150),
        billboard: {
          image: getWeatherSvg('#58a6ff'),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        label: {
          text: ` ${tempC}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString('#58a6ff'),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'weather',
          observation: w,
          title: `Weather: ${station} (${tempC})`,
          subtitle: `Temp: ${tempC} | Provider: ${w.provider}`,
          provider: w.provider,
        }),
      });
    }
  }

  /**
   * Updates or inserts wildfire thermal anomaly markers (NASA FIRMS).
   */
  public upsertWildfire(wf: Observation) {
    if (wf.latitude === undefined || wf.longitude === undefined) return;

    const id = `wildfire_${wf.entity_id ?? Math.random()}`;
    const frp = wf.attributes?.frp !== undefined ? `${wf.attributes.frp} MW` : '—';
    const confidence = wf.attributes?.confidence ? `${wf.attributes.confidence}%` : 'Nominal';

    const existing = this.wildfireDataSource.entities.getById(id);
    if (!existing) {
      this.wildfireDataSource.entities.add({
        id,
        name: `Wildfire: ${wf.entity_id ?? 'Thermal Anomaly'}`,
        position: Cesium.Cartesian3.fromDegrees(wf.longitude, wf.latitude, 150),
        billboard: {
          image: getWildfireSvg('#ff5722'),
          width: 22,
          height: 22,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'wildfire',
          observation: wf,
          title: `Wildfire Thermal Anomaly: ${wf.entity_id ?? 'FIRMS'}`,
          subtitle: `FRP: ${frp} | Confidence: ${confidence}`,
          provider: wf.provider,
        }),
      });
    }
  }

  public renderWildfires(wildfires: Observation[]) {
    this.wildfireDataSource.entities.removeAll();
    for (const wf of wildfires) {
      this.upsertWildfire(wf);
    }
  }

  /**
   * Updates or inserts air quality station markers (OpenAQ).
   */
  public upsertAirQuality(aq: Observation) {
    if (aq.latitude === undefined || aq.longitude === undefined) return;

    const id = `aqi_${aq.entity_id ?? Math.random()}`;
    const aqi = aq.metric_value !== undefined ? Math.round(aq.metric_value) : undefined;
    const aqiStr = aqi !== undefined ? `${aqi} AQI` : '—';
    const station = aq.entity_id ?? 'Air Quality Station';

    let colorHex = '#3fb950'; // Green (< 50)
    if (aqi !== undefined) {
      if (aqi > 150) colorHex = '#f85149'; // Alert red
      else if (aqi > 100) colorHex = '#f0883e'; // Orange
      else if (aqi > 50) colorHex = '#d29922'; // Amber
    }

    const existing = this.airQualityDataSource.entities.getById(id);
    if (!existing) {
      this.airQualityDataSource.entities.add({
        id,
        name: `Air Quality: ${station}`,
        position: Cesium.Cartesian3.fromDegrees(aq.longitude, aq.latitude, 150),
        billboard: {
          image: getAirQualitySvg(colorHex),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        label: {
          text: ` ${aqiStr}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString(colorHex),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'air_quality',
          observation: aq,
          title: `Air Quality: ${station} (${aqiStr})`,
          subtitle: `AQI: ${aqiStr} | Provider: ${aq.provider}`,
          provider: aq.provider,
        }),
      });
    }
  }

  public renderAirQuality(aqList: Observation[]) {
    this.airQualityDataSource.entities.removeAll();
    for (const aq of aqList) {
      this.upsertAirQuality(aq);
    }
  }

  /**
   * Updates or inserts river hydrology stream gauges (USGS Water).
   */
  public upsertHydro(h: Observation) {
    if (h.latitude === undefined || h.longitude === undefined) return;

    const id = `hydro_${h.entity_id ?? Math.random()}`;
    const stageFt = h.metric_value !== undefined ? `${h.metric_value.toFixed(1)} ft` : '—';
    const station = h.attributes?.station_name || h.entity_id || 'USGS River Gauge';

    const existing = this.hydroDataSource.entities.getById(id);
    if (!existing) {
      this.hydroDataSource.entities.add({
        id,
        name: `Hydro: ${station}`,
        position: Cesium.Cartesian3.fromDegrees(h.longitude, h.latitude, 150),
        billboard: {
          image: getHydroSvg('#03a9f4'),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'hydro',
          observation: h,
          title: `USGS Hydrology: ${station}`,
          subtitle: `Gage Height: ${stageFt} | Provider: ${h.provider}`,
          provider: h.provider,
        }),
      });
    }
  }

  public renderHydro(hydroList: Observation[]) {
    this.hydroDataSource.entities.removeAll();
    for (const h of hydroList) {
      this.upsertHydro(h);
    }
  }

  /**
   * Updates or inserts marine buoy oceanographic markers (NOAA NDBC).
   */
  public upsertBuoy(b: Observation) {
    if (b.latitude === undefined || b.longitude === undefined) return;

    const id = `buoy_${b.entity_id ?? Math.random()}`;
    const waveM = b.attributes?.wave_height_m !== undefined ? `${b.attributes.wave_height_m} m` : '—';
    const station = b.entity_id ?? 'Marine Buoy';

    const existing = this.buoyDataSource.entities.getById(id);
    if (!existing) {
      this.buoyDataSource.entities.add({
        id,
        name: `Buoy: ${station}`,
        position: Cesium.Cartesian3.fromDegrees(b.longitude, b.latitude, 100),
        billboard: {
          image: getBuoySvg('#26a69a'),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'buoy',
          observation: b,
          title: `Marine Buoy: ${station}`,
          subtitle: `Significant Wave Ht: ${waveM} | Provider: ${b.provider}`,
          provider: b.provider,
        }),
      });
    }
  }

  public renderBuoys(buoyList: Observation[]) {
    this.buoyDataSource.entities.removeAll();
    for (const b of buoyList) {
      this.upsertBuoy(b);
    }
  }

  /**
   * Updates or inserts a tropical cyclone / hurricane marker (NOAA NHC).
   */
  public upsertCyclone(c: Observation) {
    if (c.latitude === undefined || c.longitude === undefined) return;

    const id = `cyclone_${c.entity_id ?? Math.random()}`;
    const name = c.attributes?.name || c.entity_id || 'Cyclone';
    const classification = c.attributes?.classification || 'TC';
    const windKts = c.metric_value !== undefined ? Math.round(c.metric_value) : 65;
    const pressure = c.attributes?.pressure_mb ? `${c.attributes.pressure_mb} mb` : '—';

    // Color code based on Saffir-Simpson or tropical classification
    let colorHex = '#388bfd'; // Blue / Tropical Storm (< 64 kts)
    if (windKts >= 115) {
      colorHex = '#f85149'; // Cat 4-5 Major Hurricane (Red)
    } else if (windKts >= 83) {
      colorHex = '#f0883e'; // Cat 2-3 Hurricane (Orange)
    } else if (windKts >= 64) {
      colorHex = '#d29922'; // Cat 1 Hurricane (Amber)
    }

    const position = Cesium.Cartesian3.fromDegrees(c.longitude, c.latitude, 200);
    const existing = this.cycloneDataSource.entities.getById(id);

    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
    } else {
      this.cycloneDataSource.entities.add({
        id,
        name: `Cyclone ${name}`,
        position,
        billboard: {
          image: getCycloneSvg(colorHex),
          width: 24,
          height: 24,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -60),
        },
        label: {
          text: ` ${name} (${windKts} kts)`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString(colorHex),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(14, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -60),
        },
        properties: new Cesium.PropertyBag({
          layer: 'cyclone',
          observation: c,
          title: `Cyclone: ${name} (${classification})`,
          subtitle: `Max Winds: ${windKts} kts | Pressure: ${pressure} | NOAA NHC`,
          provider: c.provider,
        }),
      });
    }
  }

  public renderCyclones(cyclones: Observation[]) {
    this.cycloneDataSource.entities.removeAll();
    for (const c of cyclones) {
      this.upsertCyclone(c);
    }
  }

  /**
   * Updates or inserts an active volcano marker (USGS VHP).
   */
  public upsertVolcano(v: Observation) {
    if (v.latitude === undefined || v.longitude === undefined) return;

    const id = `volcano_${v.entity_id ?? Math.random()}`;
    const name = v.attributes?.name || 'Volcano';
    const elevM = v.altitude_m ?? v.attributes?.elevation_m ?? 0;
    const colorCode = v.attributes?.color_code || 'GREEN';
    const alertLevel = v.attributes?.alert_level || 'NORMAL';

    let colorHex = '#3fb950'; // Green
    if (colorCode === 'RED') {
      colorHex = '#f85149';
    } else if (colorCode === 'ORANGE') {
      colorHex = '#f0883e';
    } else if (colorCode === 'YELLOW') {
      colorHex = '#d29922';
    }

    const position = Cesium.Cartesian3.fromDegrees(v.longitude, v.latitude, elevM > 0 ? elevM : 200);
    const existing = this.volcanoDataSource.entities.getById(id);

    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
    } else {
      this.volcanoDataSource.entities.add({
        id,
        name: `Volcano: ${name}`,
        position,
        billboard: {
          image: getVolcanoSvg(colorHex),
          width: 22,
          height: 22,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        label: {
          text: ` ${name}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString(colorHex),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, 10_000_000),
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'volcano',
          observation: v,
          title: `Volcano: ${name}`,
          subtitle: `Aviation Code: ${colorCode} | Alert: ${alertLevel} | Elev: ${Math.round(elevM).toLocaleString()} m`,
          provider: v.provider,
        }),
      });
    }
  }

  public renderVolcanoes(volcanoes: Observation[]) {
    this.volcanoDataSource.entities.removeAll();
    for (const v of volcanoes) {
      this.upsertVolcano(v);
    }
  }

  /**
   * Renders global submarine fiber-optic cables and coastal landing stations (TeleGeography).
   */
  public renderSubmarineCables(cables: SubmarineCable[]) {
    this.cableDataSource.entities.removeAll();

    for (const cable of cables) {
      if (!cable.coordinates || cable.coordinates.length < 2) continue;

      const flatCoords: number[] = [];
      for (const pt of cable.coordinates) {
        flatCoords.push(pt[0], pt[1]);
      }

      // Add Undersea Cable Polyline with glowing cyan fiber material
      this.cableDataSource.entities.add({
        id: `cable_${cable.id}`,
        name: cable.name,
        polyline: {
          positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
          width: 2.5,
          material: new Cesium.PolylineGlowMaterialProperty({
            glowPower: 0.25,
            color: Cesium.Color.fromCssColorString('#00e5ff'),
          }),
          clampToGround: true,
        },
        properties: new Cesium.PropertyBag({
          isCable: true,
          cable,
          cableName: cable.name,
          cableType: cable.type,
          cableLengthKm: cable.length_km,
          cableOwners: cable.owners.join(', '),
          landingPointsCount: cable.landing_points.length,
          title: `Subsea Cable: ${cable.name}`,
          subtitle: `${cable.type} | Length: ${cable.length_km ? cable.length_km.toLocaleString() + ' km' : 'Trans-Oceanic'}`,
        }),
      });

      // Add Landing Stations for this cable
      for (const lp of cable.landing_points) {
        this.cableDataSource.entities.add({
          id: `cable_landing_${lp.id}`,
          name: `${lp.name} Landing Station`,
          position: Cesium.Cartesian3.fromDegrees(lp.lon, lp.lat, 100),
          billboard: {
            image: getCableLandingSvg('#00e5ff'),
            width: 18,
            height: 18,
            verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
            horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
            eyeOffset: new Cesium.Cartesian3(0, 0, -50),
          },
          label: {
            text: ` ${lp.name}`,
            font: '9px monospace',
            fillColor: Cesium.Color.fromCssColorString('#00e5ff'),
            outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
            outlineWidth: 2,
            style: Cesium.LabelStyle.FILL_AND_OUTLINE,
            pixelOffset: new Cesium.Cartesian2(10, -2),
            distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, 3_000_000),
            eyeOffset: new Cesium.Cartesian3(0, 0, -50),
          },
          properties: new Cesium.PropertyBag({
            isLandingPoint: true,
            landingPoint: lp,
            cableName: cable.name,
            title: `Landing Station: ${lp.name}`,
            subtitle: `Terminal for ${cable.name} (${lp.country})`,
          }),
        });
      }
    }
  }

  /**
   * Updates or inserts ambient radiation sensor markers (Safecast).
   */
  public upsertRadiation(r: Observation) {
    if (r.latitude === undefined || r.longitude === undefined) return;

    const id = `radiation_${r.entity_id ?? Math.random()}`;
    const usv = r.metric_value !== undefined ? r.metric_value : 0.1;
    const deviceId = r.attributes?.device_id || 'bGeigie';

    let colorHex = '#3fb950'; // Green (< 0.20 uSv/h normal background)
    if (usv > 0.50) {
      colorHex = '#f85149'; // Red (Elevated)
    } else if (usv > 0.20) {
      colorHex = '#d29922'; // Amber
    }

    const position = Cesium.Cartesian3.fromDegrees(r.longitude, r.latitude, 150);
    const existing = this.radiationDataSource.entities.getById(id);

    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
    } else {
      this.radiationDataSource.entities.add({
        id,
        name: `Radiation: ${usv.toFixed(3)} uSv/h`,
        position,
        billboard: {
          image: getRadiationSvg(colorHex),
          width: 20,
          height: 20,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        label: {
          text: ` ${usv.toFixed(2)} uSv/h`,
          font: '9px monospace',
          fillColor: Cesium.Color.fromCssColorString(colorHex),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
        },
        properties: new Cesium.PropertyBag({
          layer: 'radiation',
          observation: r,
          title: `Radiation: ${usv.toFixed(3)} uSv/h`,
          subtitle: `Sensor: ${deviceId} | Safecast CC0 Public Domain`,
          provider: r.provider,
        }),
      });
    }
  }

  public renderRadiation(radList: Observation[]) {
    this.radiationDataSource.entities.removeAll();
    for (const r of radList) {
      this.upsertRadiation(r);
    }
  }

  /**
   * Renders the NOAA SWPC Ovation Aurora oval across the polar ionosphere.
   */
  public renderAurora(auroraPoints: Observation[]) {
    this.auroraDataSource.entities.removeAll();

    for (const pt of auroraPoints) {
      if (pt.latitude === undefined || pt.longitude === undefined) continue;
      const prob = pt.metric_value ?? 30.0;
      const alpha = Math.max(0.2, Math.min(0.7, prob / 100.0));

      this.auroraDataSource.entities.add({
        id: `aurora_${pt.entity_id ?? Math.random()}`,
        name: `Aurora Ovation (${Math.round(prob)}%)`,
        position: Cesium.Cartesian3.fromDegrees(pt.longitude, pt.latitude, 110_000),
        point: {
          pixelSize: Math.max(6, Math.min(16, Math.round(prob * 0.2))),
          color: Cesium.Color.fromCssColorString('#50fa7b').withAlpha(alpha),
          outlineColor: Cesium.Color.fromCssColorString('#bd93f9').withAlpha(alpha * 0.7),
          outlineWidth: 2,
        },
        properties: new Cesium.PropertyBag({
          layer: 'aurora',
          observation: pt,
          title: `Aurora Probability: ${Math.round(prob)}%`,
          subtitle: `Ionospheric Glow (~110 km) | NOAA SWPC Ovation`,
          provider: pt.provider,
        }),
      });
    }
  }

  /**
   * Updates or inserts a humanitarian crisis / disaster marker (GDACS).
   */
  public upsertDisaster(d: Observation) {
    if (d.latitude === undefined || d.longitude === undefined) return;

    const id = `disaster_${d.entity_id ?? Math.random()}`;
    const name = d.attributes?.event_name || 'Disaster Alert';
    const alertLevel = d.attributes?.alert_level || 'Orange';
    const eventType = d.attributes?.event_type || 'Crisis';

    let colorHex = '#f0883e'; // Orange
    if (alertLevel.toLowerCase() === 'red') {
      colorHex = '#f85149';
    } else if (alertLevel.toLowerCase() === 'green') {
      colorHex = '#3fb950';
    }

    const position = Cesium.Cartesian3.fromDegrees(d.longitude, d.latitude, 200);
    const existing = this.disasterDataSource.entities.getById(id);

    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(position);
    } else {
      this.disasterDataSource.entities.add({
        id,
        name: `Crisis: ${name}`,
        position,
        billboard: {
          image: getDisasterSvg(colorHex),
          width: 24,
          height: 24,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          horizontalOrigin: Cesium.HorizontalOrigin.CENTER,
          eyeOffset: new Cesium.Cartesian3(0, 0, -60),
        },
        label: {
          text: ` [${eventType}] ${name}`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString(colorHex),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(14, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -60),
        },
        properties: new Cesium.PropertyBag({
          layer: 'disaster',
          observation: d,
          title: `Crisis: ${name} (${eventType})`,
          subtitle: `Alert: ${alertLevel} | Country: ${d.attributes?.country ?? 'Global'} | GDACS`,
          provider: d.provider,
        }),
      });
    }
  }

  public renderDisasters(disasters: Observation[]) {
    this.disasterDataSource.entities.removeAll();
    for (const d of disasters) {
      this.upsertDisaster(d);
    }
  }

  public updateSpaceWeatherDisplay(obs: Observation) {
    const kp = obs.metric_value ?? 0;
    const kpEl = document.getElementById('kp-gauge-value');
    if (kpEl) {
      kpEl.textContent = `Kp ${kp.toFixed(1)}`;
      kpEl.className = kp >= 5 ? 'badge-alert' : 'badge-normal';
    }
  }

  public updatePowerGridDisplay(obs: Observation) {
    const renewPerc = obs.attributes?.renewable_total_perc ?? 0;
    const fossilPerc = obs.attributes?.fossil_total_perc ?? 0;
    const gridEl = document.getElementById('grid-mix-value');
    if (gridEl) {
      gridEl.textContent = `Renewable: ${renewPerc}% | Fossil: ${fossilPerc}%`;
    }
  }

  /**
   * Renders structured key-value metric tiles into the breakdown grid.
   */
  public renderBreakdownTiles(tiles: Array<{ label: string; value: string; highlight?: 'cyan' | 'green' | 'orange' | 'red'; full?: boolean }>) {
    const grid = document.getElementById('entity-breakdown-grid');
    if (!grid) return;
    grid.innerHTML = '';

    for (const t of tiles) {
      const tile = document.createElement('div');
      tile.className = `breakdown-tile ${t.full ? 'tile-full' : ''}`;

      const label = document.createElement('span');
      label.className = 'breakdown-tile-label';
      label.textContent = t.label;

      const val = document.createElement('span');
      const hlClass = t.highlight ? `highlight-${t.highlight}` : '';
      val.className = `breakdown-tile-value ${hlClass}`;
      val.textContent = t.value;

      tile.appendChild(label);
      tile.appendChild(val);
      grid.appendChild(tile);
    }
  }

  /**
   * Updates the selected breakdown header (badge, icon, title, subtitle).
   */
  public updateSelectionHeader(badgeText: string, icon: string, title: string, subtitle: string) {
    const badgeEl = document.getElementById('selected-layer-badge');
    const iconEl = document.getElementById('selected-entity-icon');
    const nameEl = document.getElementById('selected-entity-name');
    const detailsEl = document.getElementById('selected-entity-details');

    if (badgeEl) badgeEl.textContent = badgeText;
    if (iconEl) iconEl.textContent = icon;
    if (nameEl) nameEl.textContent = title;
    if (detailsEl) detailsEl.textContent = subtitle;
  }

  /**
   * Displays an exhaustive telemetry and attribute breakdown for a clicked entity.
   */
  public displayEntityBreakdown(obs: Observation, props: any) {
    const title = props?.title?.getValue() || obs.entity_id || 'Entity';
    const subtitle = props?.subtitle?.getValue() || `${obs.provider} telemetry`;
    const lat = obs.latitude ?? 0;
    const lon = obs.longitude ?? 0;
    const altM = obs.altitude_m ?? 0;
    const attrs = obs.attributes || {};

    switch (obs.layer) {
      case 'aircraft': {
        const callsign = attrs.flight?.trim() || obs.entity_id || 'AIRCRAFT';
        const track = attrs.track ?? 0;
        const gs = attrs.gs !== undefined ? Math.round(attrs.gs) : undefined;
        const squawk = attrs.squawk ?? '----';
        const hex = attrs.hex ?? obs.entity_id ?? '—';
        const altFt = Math.round(altM * 3.28084);

        this.updateSelectionHeader('AIRCRAFT (ADS-B)', '✈', `Flight ${callsign}`, `Hex: ${hex} | Provider: ${obs.provider}`);
        this.renderBreakdownTiles([
          { label: 'Altitude', value: `${Math.round(altM).toLocaleString()} m (${altFt.toLocaleString()} ft)`, highlight: 'cyan' },
          { label: 'Ground Speed', value: gs ? `${gs} kts (${Math.round(gs * 1.852)} km/h)` : '—' },
          { label: 'Heading / Track', value: `${track}° ${getCompassHeading(track)}` },
          { label: 'Transponder Squawk', value: squawk, highlight: squawk.startsWith('7') ? 'red' : 'green' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Data Source', value: `${obs.provider.toUpperCase()} (ADS-B)` },
        ]);
        break;
      }
      case 'ship': {
        const name = attrs.name?.trim() || `MMSI-${obs.entity_id}`;
        const sog = attrs.sog;
        const cog = attrs.cog;
        const navStatus = attrs.nav_status;
        const mmsi = obs.entity_id || '—';

        this.updateSelectionHeader('VESSEL (AIS)', '🚢', `Vessel ${name}`, `MMSI: ${mmsi} | Provider: ${obs.provider}`);
        this.renderBreakdownTiles([
          { label: 'MMSI Identifier', value: mmsi, highlight: 'cyan' },
          { label: 'Speed Over Ground', value: sog !== undefined ? `${Number(sog).toFixed(1)} kts` : '—' },
          { label: 'Course Over Ground', value: cog !== undefined ? `${Number(cog).toFixed(0)}°` : '—' },
          { label: 'Navigational Status', value: navStatus ? String(navStatus) : 'Underway' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Maritime Telemetry', value: `${obs.provider.toUpperCase()} (AIS)` },
        ]);
        break;
      }
      case 'satellite': {
        const satName = attrs.name ?? `SAT-${obs.entity_id}`;
        const altKm = Math.round(altM / 1000);
        const noradId = obs.entity_id || '—';

        this.updateSelectionHeader('SATELLITE (SGP4)', '🛰', `Satellite ${satName}`, `NORAD ID: ${noradId} | Orbit: LEO`);
        this.renderBreakdownTiles([
          { label: 'Altitude (Orbital)', value: `${altKm.toLocaleString()} km`, highlight: 'cyan' },
          { label: 'NORAD Catalog ID', value: noradId },
          { label: 'Orbital Velocity', value: '~7.66 km/s (27,600 km/h)' },
          { label: 'Orbital Period', value: '~92.8 min' },
          { label: 'Sub-Satellite Point', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Ephemeris Model', value: 'SGP4 / Bowring WGS84' },
        ]);
        break;
      }
      case 'quake': {
        const mag = obs.metric_value ?? 0;
        const depthKm = obs.altitude_m !== undefined ? Math.abs(obs.altitude_m / 1000).toFixed(1) : '0';
        const place = attrs.place ?? 'Seismic Zone';
        const felt = attrs.felt;
        const timeStr = new Date(obs.timestamp).toISOString().substring(11, 19);

        this.updateSelectionHeader('SEISMIC EVENT', '⚡', `Earthquake M ${mag.toFixed(1)}`, `${place}`);
        this.renderBreakdownTiles([
          { label: 'Magnitude (Richter)', value: `M ${mag.toFixed(1)}`, highlight: mag >= 6 ? 'red' : 'orange' },
          { label: 'Hypocenter Depth', value: `${depthKm} km` },
          { label: 'Epicenter Coordinates', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Felt Intensity', value: felt ? `${felt} felt reports` : 'Instrumental' },
          { label: 'Origin Time (UTC)', value: `${timeStr} UTC` },
          { label: 'Seismic Network', value: `${obs.provider.toUpperCase()} Hazards` },
        ]);
        break;
      }
      case 'weather': {
        const tempC = obs.metric_value !== undefined ? `${obs.metric_value.toFixed(1)}°C` : '—';
        const station = obs.entity_id ?? 'Weather Station';

        this.updateSelectionHeader('WEATHER SENSOR', '⛅', `Weather: ${station}`, `Surface Meteorological Telemetry`);
        this.renderBreakdownTiles([
          { label: 'Observed Temp', value: tempC, highlight: 'cyan' },
          { label: 'Station Identifier', value: station },
          { label: 'Observation Layer', value: 'Surface Meteorology' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Model Provider', value: `${obs.provider.toUpperCase()} High-Res` },
          { label: 'Station Status', value: 'Active Live Stream', highlight: 'green' },
        ]);
        break;
      }
      case 'wildfire': {
        const frp = attrs.frp !== undefined ? `${attrs.frp} MW` : '—';
        const conf = attrs.confidence ? `${attrs.confidence}%` : 'Nominal';
        const bright = attrs.brightness ? `${attrs.brightness} K` : '—';

        this.updateSelectionHeader('WILDFIRE (NASA FIRMS)', '🔥', `Thermal Anomaly: ${obs.entity_id || 'FIRMS'}`, `NASA Active Fire Telemetry`);
        this.renderBreakdownTiles([
          { label: 'Thermal Anomaly ID', value: obs.entity_id || '—', highlight: 'orange' },
          { label: 'Fire Radiative Power', value: frp, highlight: 'red' },
          { label: 'Detection Confidence', value: conf, highlight: 'green' },
          { label: 'Brightness Temp', value: bright },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Sensor Constellation', value: `${obs.provider.toUpperCase()} (MODIS/VIIRS)` },
        ]);
        break;
      }
      case 'air_quality': {
        const aqiVal = obs.metric_value !== undefined ? String(Math.round(obs.metric_value)) : '—';
        const station = obs.entity_id ?? 'Air Quality Station';
        const param = attrs.parameter || 'PM2.5';

        this.updateSelectionHeader('AIR QUALITY SENSOR', '💨', `Air Quality: ${station}`, `OpenAQ Environmental Monitoring`);
        this.renderBreakdownTiles([
          { label: 'AQI Index', value: aqiVal, highlight: (obs.metric_value ?? 0) > 100 ? 'red' : 'green' },
          { label: 'Station Name', value: station },
          { label: 'Monitored Parameter', value: param, highlight: 'cyan' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Observation Source', value: `${obs.provider.toUpperCase()} Network` },
        ]);
        break;
      }
      case 'hydro': {
        const stage = obs.metric_value !== undefined ? `${obs.metric_value.toFixed(2)} ft` : '—';
        const flow = attrs.flow_cfs ? `${Number(attrs.flow_cfs).toLocaleString()} cfs` : '—';
        const station = attrs.station_name || obs.entity_id || 'USGS River Gauge';

        this.updateSelectionHeader('HYDROLOGY GAUGE', '💧', `Stream: ${station}`, `USGS Real-Time Water Services`);
        this.renderBreakdownTiles([
          { label: 'Gage Height / Stage', value: stage, highlight: 'cyan' },
          { label: 'Discharge Flow', value: flow },
          { label: 'Station Identifier', value: obs.entity_id || '—' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Telemetry Provider', value: `${obs.provider.toUpperCase()} Hydrology` },
        ]);
        break;
      }
      case 'buoy': {
        const wave = attrs.wave_height_m ? `${attrs.wave_height_m} m` : '—';
        const waterTemp = attrs.water_temp_c ? `${attrs.water_temp_c} °C` : '—';
        const wind = attrs.wind_speed_kts ? `${attrs.wind_speed_kts} kts` : '—';
        const station = obs.entity_id || 'Marine Buoy';

        this.updateSelectionHeader('MARINE BUOY (NDBC)', '⚓', `Buoy: ${station}`, `NOAA National Data Buoy Center`);
        this.renderBreakdownTiles([
          { label: 'Significant Wave Height', value: wave, highlight: 'cyan' },
          { label: 'Sea Surface Temp', value: waterTemp },
          { label: 'Wind Speed', value: wind },
          { label: 'Buoy Station ID', value: station },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Oceanographic Agency', value: `${obs.provider.toUpperCase()} NDBC` },
        ]);
        break;
      }
      case 'cyclone': {
        const windKts = obs.metric_value !== undefined ? Math.round(obs.metric_value) : 65;
        const pressure = attrs.pressure_mb ? `${attrs.pressure_mb} mb` : '—';
        const classification = attrs.classification || 'TC';
        const name = attrs.name || obs.entity_id || 'Cyclone';

        this.updateSelectionHeader('TROPICAL CYCLONE (NHC)', '🌪', `Cyclone ${name}`, `Classification: ${classification} | NOAA NHC`);
        this.renderBreakdownTiles([
          { label: 'Max Sustained Winds', value: `${windKts} kts (${Math.round(windKts * 1.852)} km/h)`, highlight: windKts >= 100 ? 'red' : 'orange' },
          { label: 'Central Pressure', value: pressure },
          { label: 'Storm Classification', value: classification, highlight: 'cyan' },
          { label: 'Movement / Track', value: attrs.movement_heading_deg !== undefined ? `${attrs.movement_heading_deg}° at ${attrs.movement_speed_kts ?? 0} kts` : 'Active Trajectory' },
          { label: 'Eye Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Meteorological Agency', value: 'NOAA National Hurricane Center' },
        ]);
        break;
      }
      case 'volcano': {
        const colorCode = attrs.color_code || 'GREEN';
        const alertLevel = attrs.alert_level || 'NORMAL';
        const name = attrs.name || 'Volcano';
        const elev = attrs.elevation_m ? `${Math.round(attrs.elevation_m).toLocaleString()} m` : '—';

        this.updateSelectionHeader('ACTIVE VOLCANO (USGS)', '🌋', `Volcano: ${name}`, `Aviation Threat Code: ${colorCode}`);
        this.renderBreakdownTiles([
          { label: 'Aviation Color Code', value: colorCode, highlight: colorCode === 'RED' ? 'red' : (colorCode === 'ORANGE' ? 'orange' : 'green') },
          { label: 'Volcano Alert Level', value: alertLevel, highlight: 'cyan' },
          { label: 'Summit Elevation', value: elev },
          { label: 'Observatory Group', value: attrs.observatory || 'USGS VHP' },
          { label: 'Summit Coordinates', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Monitoring Agency', value: 'USGS Volcano Hazards Program' },
        ]);
        break;
      }
      case 'submarine_cable': {
        this.updateSelectionHeader('SUBSEA FIBER CABLE', '🌐', title, subtitle);
        this.renderBreakdownTiles([
          { label: 'Cable System', value: title, highlight: 'cyan' },
          { label: 'Undersea Line Layer', value: 'Intercontinental Optical Mesh', highlight: 'green' },
          { label: 'Telemetry Source', value: 'TeleGeography Open Data' },
          { label: 'Coordinates (WGS84)', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
        ]);
        break;
      }
      case 'radiation': {
        const usv = obs.metric_value !== undefined ? obs.metric_value : 0.1;
        const device = attrs.device_id || 'bGeigie';

        this.updateSelectionHeader('RADIATION MONITOR (SAFECAST)', '☢', `Ambient Dose: ${usv.toFixed(3)} uSv/h`, `Safecast Open Environmental Grid`);
        this.renderBreakdownTiles([
          { label: 'Ambient Dose Rate', value: `${usv.toFixed(3)} uSv/h`, highlight: usv > 0.5 ? 'red' : (usv > 0.2 ? 'orange' : 'green') },
          { label: 'Radiation Assessment', value: usv < 0.25 ? 'Normal Environmental Background' : 'Elevated Reading Detected', highlight: usv < 0.25 ? 'green' : 'orange' },
          { label: 'Sensor Hardware', value: device },
          { label: 'Raw Reading', value: attrs.raw_value ? `${attrs.raw_value} ${attrs.unit ?? 'cpm'}` : '—' },
          { label: 'Sensor Coordinates', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Data Provenance', value: 'Safecast CC0 Universal Public Domain' },
        ]);
        break;
      }
      case 'aurora': {
        const prob = obs.metric_value !== undefined ? Math.round(obs.metric_value) : 30;

        this.updateSelectionHeader('AURORA OVATION (NOAA)', '🌌', `Aurora Probability: ${prob}%`, `NOAA SWPC Ovation 30-Min Forecast`);
        this.renderBreakdownTiles([
          { label: 'Aurora Probability', value: `${prob}%`, highlight: 'green' },
          { label: 'Hemisphere', value: attrs.hemisphere ? `${attrs.hemisphere} Pole` : 'Polar Auroral Oval', highlight: 'cyan' },
          { label: 'Ionospheric Altitude', value: '~110 - 120 km Upper Atmosphere' },
          { label: 'Grid Coordinates', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Space Weather Model', value: 'NOAA SWPC Ovation Real-Time' },
          { label: 'Observation Status', value: 'Live Geomagnetic Forecast' },
        ]);
        break;
      }
      case 'disaster': {
        const eventName = attrs.event_name || 'Crisis Alert';
        const eventType = attrs.event_type || 'Crisis';
        const alertLevel = attrs.alert_level || 'Orange';
        const country = attrs.country || 'Global';

        this.updateSelectionHeader('CRISIS ALERT (GDACS)', '⚠', `Disaster: ${eventName}`, `Severity: ${alertLevel} | GDACS / UN OCHA`);
        this.renderBreakdownTiles([
          { label: 'Disaster Classification', value: eventType, highlight: 'cyan' },
          { label: 'Alert Severity', value: alertLevel, highlight: alertLevel.toLowerCase() === 'red' ? 'red' : 'orange' },
          { label: 'Impacted Territory', value: country, highlight: 'green' },
          { label: 'Event Description', value: attrs.description || eventName, full: true },
          { label: 'Event Epicenter', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Coordinating Agency', value: 'UN OCHA & European Commission JRC' },
        ]);
        break;
      }
      default: {
        this.updateSelectionHeader(obs.layer.toUpperCase(), '◈', title, subtitle);
        this.renderBreakdownTiles([
          { label: 'Entity ID', value: obs.entity_id || '—' },
          { label: 'Observation Layer', value: obs.layer },
          { label: 'Coordinates', value: `${lat.toFixed(4)}°, ${lon.toFixed(4)}°` },
          { label: 'Telemetry Source', value: obs.provider },
        ]);
      }
    }
  }

  /**
   * Displays an exhaustive geographic hierarchy breakdown when clicking on any city, state, or country,
   * scoped dynamically by camera altitude matching Google Maps hierarchical behavior.
   */
  public displayGeographicBreakdown(lat: number, lon: number, hierarchy: ResolvedHierarchy, altM: number) {
    const level = getHierarchyLevelForAltitude(altM);
    const altKmStr = `${Math.round(altM / 1000).toLocaleString()} km`;

    // 1. Really zoomed out (> 4,500 km) -> Prioritize Country / Sovereign State
    if (level === 'country') {
      if (hierarchy.country) {
        this.updateSelectionHeader(
          'NATION / SOVEREIGN',
          '🌐',
          hierarchy.country,
          `Sovereign State (${hierarchy.country_code || 'ISO'}) | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: 'Nation / Sovereign', highlight: 'cyan' },
          { label: 'Country / Nation', value: `${hierarchy.country}${hierarchy.country_code ? ' (' + hierarchy.country_code + ')' : ''}`, highlight: 'green' },
          { label: 'Selection Mode', value: 'Global View (> 4,500 km)' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
          { label: 'Admin Hierarchy', value: 'Level 0 Sovereign Territory' },
          { label: 'Spatial Provenance', value: '< 1 ms offline R-Tree index' },
        ]);
        return;
      }
    }

    // 2. Zoomed in some (1,200 km - 4,500 km) -> Prioritize State / Province
    if (level === 'state') {
      if (hierarchy.state) {
        this.updateSelectionHeader(
          'STATE / PROVINCE',
          '🗺',
          `${hierarchy.state}${hierarchy.state_code ? ' (' + hierarchy.state_code + ')' : ''}`,
          `${hierarchy.country || 'Territory'} | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: 'State / Province', highlight: 'cyan' },
          { label: 'State / Province', value: `${hierarchy.state}${hierarchy.state_code ? ' (' + hierarchy.state_code + ')' : ''}`, highlight: 'green' },
          { label: 'Nation / Sovereign', value: hierarchy.country || '—' },
          { label: 'Selection Mode', value: 'Regional View (1,200 - 4,500 km)' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
          { label: 'Spatial Provenance', value: '< 1 ms offline R-Tree index' },
        ]);
        return;
      } else if (hierarchy.country) {
        // Fallback to Country if state is not defined for this nation
        this.updateSelectionHeader(
          'NATION / SOVEREIGN',
          '🌐',
          hierarchy.country,
          `Unitary Sovereign State (${hierarchy.country_code || 'ISO'}) | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: 'Nation / Unitary State', highlight: 'cyan' },
          { label: 'Country / Nation', value: `${hierarchy.country}${hierarchy.country_code ? ' (' + hierarchy.country_code + ')' : ''}`, highlight: 'green' },
          { label: 'Subdivision Status', value: 'No State Subdivisions' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
          { label: 'Admin Hierarchy', value: 'Level 0 Sovereign Territory' },
          { label: 'Spatial Provenance', value: '< 1 ms offline R-Tree index' },
        ]);
        return;
      }
    }

    // 3. Zoomed in more (350 km - 1,200 km) -> Prioritize County / District or Metro Area
    if (level === 'county') {
      if (hierarchy.county) {
        this.updateSelectionHeader(
          'COUNTY / DISTRICT',
          '🏛',
          hierarchy.county,
          `${hierarchy.state ? hierarchy.state + ' → ' : ''}${hierarchy.country || ''} | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: 'County / District', highlight: 'cyan' },
          { label: 'County / District', value: hierarchy.county, highlight: 'green' },
          { label: 'State / Province', value: hierarchy.state || '—' },
          { label: 'Nearest Metro', value: hierarchy.city || '—' },
          { label: 'Nation / Sovereign', value: hierarchy.country || '—' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
        ]);
        return;
      } else if (hierarchy.city) {
        this.updateSelectionHeader(
          'METRO REGION',
          '🏛',
          `${hierarchy.city}${hierarchy.state_code ? ', ' + hierarchy.state_code : ''}`,
          `${hierarchy.state ? hierarchy.state + ' → ' : ''}${hierarchy.country || ''} | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: 'Metro Region', highlight: 'cyan' },
          { label: 'Metro City', value: hierarchy.city, highlight: 'green' },
          { label: 'State / Province', value: hierarchy.state || '—' },
          { label: 'Nation / Sovereign', value: hierarchy.country || '—' },
          { label: 'Selection Mode', value: 'Metro View (350 - 1,200 km)' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
        ]);
        return;
      } else if (hierarchy.state || hierarchy.country) {
        const title = hierarchy.state || hierarchy.country || 'Region';
        this.updateSelectionHeader(
          hierarchy.state ? 'STATE / PROVINCE' : 'NATION / SOVEREIGN',
          hierarchy.state ? '🗺' : '🌐',
          title,
          `${hierarchy.country || ''} | Altitude: ${altKmStr}`
        );
        this.renderBreakdownTiles([
          { label: 'Selected Scope', value: hierarchy.state ? 'State / Province' : 'Nation', highlight: 'cyan' },
          { label: 'Region', value: title, highlight: 'green' },
          { label: 'Nation / Sovereign', value: hierarchy.country || '—' },
          { label: 'County Status', value: 'No County Subdivisions' },
          { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
          { label: 'Spatial Provenance', value: '< 1 ms offline R-Tree index' },
        ]);
        return;
      }
    }

    // 4. Zoomed in close (<= 350 km) -> Prioritize exact City / Municipality
    if (hierarchy.city) {
      const breadcrumb = [hierarchy.city, hierarchy.county, hierarchy.state, hierarchy.country]
        .filter(Boolean)
        .join(' → ');
      this.updateSelectionHeader(
        'CITY / MUNICIPALITY',
        '📍',
        `${hierarchy.city}${hierarchy.state_code ? ', ' + hierarchy.state_code : ''}`,
        `${breadcrumb} | Altitude: ${altKmStr}`
      );
      this.renderBreakdownTiles([
        { label: 'Selected Scope', value: 'City / Municipality', highlight: 'cyan' },
        { label: 'City / Settlement', value: hierarchy.city, highlight: 'green' },
        { label: 'County / District', value: hierarchy.county || 'Unincorporated' },
        { label: 'State / Province', value: hierarchy.state ? `${hierarchy.state}${hierarchy.state_code ? ' (' + hierarchy.state_code + ')' : ''}` : '—' },
        { label: 'Distance to Centroid', value: hierarchy.distance_to_city_km !== undefined ? `${hierarchy.distance_to_city_km.toFixed(1)} km` : 'Direct' },
        { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
      ]);
      return;
    } else if (hierarchy.country || hierarchy.state || hierarchy.county) {
      // Unincorporated / rural settlement
      const title = hierarchy.county || hierarchy.state || hierarchy.country || 'Rural Settlement';
      this.updateSelectionHeader(
        'RURAL / UNINCORPORATED',
        '📍',
        title,
        `Unincorporated Area | ${hierarchy.state ? hierarchy.state + ' → ' : ''}${hierarchy.country || ''}`
      );
      this.renderBreakdownTiles([
        { label: 'Selected Scope', value: 'Rural / Unincorporated', highlight: 'cyan' },
        { label: 'Local District', value: hierarchy.county || 'Unorganized' },
        { label: 'State / Province', value: hierarchy.state || '—' },
        { label: 'Nation / Sovereign', value: hierarchy.country || '—' },
        { label: 'Settlement Status', value: 'No incorporated place within 25 km' },
        { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
      ]);
      return;
    }

    // 5. Open Ocean / International Waters (no nation boundary)
    this.updateSelectionHeader(
      'MARITIME / OCEAN',
      '🌊',
      'Open Ocean / International Waters',
      `WGS84: [${lat.toFixed(4)}°, ${lon.toFixed(4)}°] | Altitude: ${altKmStr}`
    );
    this.renderBreakdownTiles([
      { label: 'Selected Scope', value: 'International Waters', highlight: 'cyan' },
      { label: 'Maritime Jurisdiction', value: 'High Seas (UNCLOS Treaty)' },
      { label: 'Bathymetric Baseline', value: 'WGS84 Ellipsoidal Surface' },
      { label: 'Cartography', value: 'Tier 1 Natural Earth 1:50m' },
      { label: 'WGS84 Coordinates', value: `${lat.toFixed(4)}° N, ${lon.toFixed(4)}° E` },
      { label: 'Spatial Provenance', value: 'Sub-millisecond Offline Grid' },
    ]);
  }

  /**
   * Wires up click picking to display entity details and structured breakdown in Context Panel.
   * Dynamically resolves geographic level based on camera altitude (Google Maps style).
   * - Single click: Selects and highlights the County/City/State/Country boundary polygon.
   * - Double click: Places a precise coordinate pinpoint marker (blue dot).
   */
  private setupEntityPicking() {
    // Disable Cesium's default aggressive camera tracking on double-click
    this.viewer.screenSpaceEventHandler.removeInputAction(Cesium.ScreenSpaceEventType.LEFT_DOUBLE_CLICK);

    const handler = new Cesium.ScreenSpaceEventHandler(this.viewer.scene.canvas);

    // 1. SINGLE CLICK: Prioritized entity inspection & boundary selection
    handler.setInputAction(async (click: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      // Step A: Search for interactive objects under/near the click cursor (24x24 px pick aperture)
      let pickedObjects: any[] = [];
      try {
        pickedObjects = this.viewer.scene.drillPick(click.position, 25, 24, 24);
      } catch (err) {
        console.warn('[HUMBOLDT] drillPick warning:', err);
      }

      if (!pickedObjects || pickedObjects.length === 0) {
        const singlePick = this.viewer.scene.pick(click.position);
        if (Cesium.defined(singlePick)) {
          pickedObjects = [singlePick];
        }
      }

      // Filter out purely decorative selection entities
      const validPicks = pickedObjects.filter(
        (p: any) =>
          Cesium.defined(p) &&
          p.id &&
          p.id.id !== 'selected_boundary_fill' &&
          p.id.id !== 'selected_boundary_glow' &&
          p.id.id !== 'selected_location_marker' &&
          p.id.id !== 'selected_entity_reticle'
      );

      // Priority 1: Observation Entities (aircraft, ship, cyclone, disaster, quake, weather, wildfire, etc.)
      const obsPicks = validPicks.filter(
        (p: any) => p.id?.properties?.observation !== undefined
      );

      if (obsPicks.length > 0) {
        let bestPick = obsPicks[0];
        let minDistanceSq = Number.MAX_VALUE;

        const currentTime = this.viewer.clock.currentTime;
        for (const pick of obsPicks) {
          const ent = pick.id as Cesium.Entity;
          if (ent.position) {
            const worldPos = typeof ent.position.getValue === 'function'
              ? ent.position.getValue(currentTime)
              : ent.position;
            if (worldPos) {
              const screenPos = Cesium.SceneTransforms.worldToWindowCoordinates(
                this.viewer.scene,
                worldPos as Cesium.Cartesian3
              );
              if (screenPos) {
                const dx = screenPos.x - click.position.x;
                const dy = screenPos.y - click.position.y;
                const distSq = dx * dx + dy * dy;
                if (distSq < minDistanceSq) {
                  minDistanceSq = distSq;
                  bestPick = pick;
                }
              }
            }
          }
        }

        const pickedEntity = bestPick.id as Cesium.Entity;
        const props = pickedEntity.properties;
        const obs = props?.observation?.getValue() as Observation | undefined;

        if (obs) {
          this.selectedEntityId = pickedEntity.id;
          this.highlightSelectedEntity(pickedEntity);
          this.displayEntityBreakdown(obs, props);
          this.appendNewsDispatch(obs);
          if (obs.latitude !== undefined && obs.longitude !== undefined) {
            const articles = await fetchNews(obs.latitude, obs.longitude);
            if (articles.length > 0) {
              this.renderNewsArticles(articles);
            }
          }
          return;
        }
      }

      // Priority 2: Subsea Cables & Landing Points
      const cablePick = validPicks.find(
        (p: any) =>
          p.id?.properties?.isCable !== undefined ||
          p.id?.properties?.isLandingPoint !== undefined
      );

      if (cablePick) {
        const pickedEntity = cablePick.id as Cesium.Entity;
        const props = pickedEntity.properties;
        if (props?.isCable?.getValue()) {
          this.selectedEntityId = pickedEntity.id;
          this.highlightSelectedEntity(pickedEntity);
          const cable = props.cable?.getValue() as SubmarineCable;
          if (cable) {
            this.updateSelectionHeader('SUBSEA FIBER CABLE', '🌐', cable.name, cable.type);
            this.renderBreakdownTiles([
              { label: 'Cable System', value: cable.name, highlight: 'cyan' },
              { label: 'Optical Capacity', value: cable.type, highlight: 'green' },
              { label: 'Route Length', value: cable.length_km ? `${cable.length_km.toLocaleString()} km` : 'Trans-Oceanic' },
              { label: 'Consortium Owners', value: cable.owners.join(', ') || 'Global Telecoms' },
              { label: 'Landing Stations', value: `${cable.landing_points.length} Worldwide Terminals` },
              { label: 'Cartographic Source', value: 'TeleGeography Open Data Backbone' },
            ]);
          }
          return;
        } else if (props?.isLandingPoint?.getValue()) {
          this.selectedEntityId = pickedEntity.id;
          this.highlightSelectedEntity(pickedEntity);
          const lp = props.landingPoint?.getValue();
          const cableName = props.cableName?.getValue() || 'Subsea Cable';
          if (lp) {
            this.updateSelectionHeader('CABLE LANDING TERMINAL', '⚓', lp.name, `Terminal for ${cableName} (${lp.country})`);
            this.renderBreakdownTiles([
              { label: 'Landing Terminal', value: lp.name, highlight: 'cyan' },
              { label: 'Host Sovereign Nation', value: lp.country, highlight: 'green' },
              { label: 'Connected Cable', value: cableName },
              { label: 'Coordinates (WGS84)', value: `${lp.lat.toFixed(4)}°, ${lp.lon.toFixed(4)}°` },
              { label: 'Terminal Role', value: 'International Optical Exchange Hub' },
              { label: 'Infrastructure Registry', value: 'TeleGeography Open Backbone' },
            ]);
          }
          return;
        }
      }

      // Priority 3: Populated Place / City Markers
      const placePick = validPicks.find(
        (p: any) => p.id?.properties?.isPlace !== undefined
      );

      if (placePick) {
        const pickedEntity = placePick.id as Cesium.Entity;
        const props = pickedEntity.properties;
        this.selectedEntityId = pickedEntity.id;
        const pLat = props?.lat?.getValue();
        const pLon = props?.lon?.getValue();
        const placeName = props?.placeName?.getValue();
        const altM = this.viewer.camera.positionCartographic.height;

        let cityPoly: GeoPolygon | null =
          this.boundaries?.cities?.find(c => c.name.toLowerCase() === (placeName || '').toLowerCase()) || null;
        if (!cityPoly && pLat !== undefined && pLon !== undefined) {
          cityPoly = this.findPolygonForCoordinate(pLat, pLon, 'city');
        }
        if (pLat !== undefined && pLon !== undefined) {
          this.highlightSelectedBoundary(cityPoly, { lat: pLat, lon: pLon }, 'city');
          const hierarchy = await resolveGeo(pLat, pLon, 'city');
          this.displayGeographicBreakdown(pLat, pLon, hierarchy, altM);
          const articles = await fetchNews(
            pLat,
            pLon,
            hierarchy.city || hierarchy.county,
            hierarchy.state,
            hierarchy.country,
            'city'
          );
          if (articles.length > 0) {
            this.renderNewsArticles(articles);
          }
        }
        return;
      }

      // Priority 4: Direct click on a boundary outline line
      const boundaryPick = validPicks.find(
        (p: any) => p.id?.properties?.isBoundary !== undefined
      );

      if (boundaryPick) {
        const pickedEntity = boundaryPick.id as Cesium.Entity;
        const props = pickedEntity.properties;
        const poly = props?.polygon?.getValue() as GeoPolygon;
        const level = (props?.boundaryLevel?.getValue() || 'region') as GeoHierarchyLevel;
        if (poly) {
          this.selectedEntityId = pickedEntity.id;
          this.highlightSelectedBoundary(poly);
          const centerLat = (poly.min_lat + poly.max_lat) / 2;
          const centerLon = (poly.min_lon + poly.max_lon) / 2;
          const altM = this.viewer.camera.positionCartographic.height;
          const hierarchy = await resolveGeo(centerLat, centerLon, level);
          this.displayGeographicBreakdown(centerLat, centerLon, hierarchy, altM);
          const articles = await fetchNews(
            centerLat,
            centerLon,
            hierarchy.city || hierarchy.county,
            hierarchy.state,
            hierarchy.country,
            level
          );
          if (articles.length > 0) {
            this.renderNewsArticles(articles);
          }
          return;
        }
      }

      // Priority 5: Globe surface coordinate click (Territory / Ocean resolution)
      this.selectedEntityId = null;
      const oldReticle = this.reticleDataSource.entities.getById('selected_entity_reticle');
      if (oldReticle) {
        this.reticleDataSource.entities.remove(oldReticle);
      }

      const cartesian = this.viewer.camera.pickEllipsoid(
        click.position,
        this.viewer.scene.globe.ellipsoid
      );

      if (cartesian) {
        const cartographic = Cesium.Cartographic.fromCartesian(cartesian);
        const lat = Cesium.Math.toDegrees(cartographic.latitude);
        const lon = Cesium.Math.toDegrees(cartographic.longitude);
        const altM = this.viewer.camera.positionCartographic.height;
        const targetLevel = getHierarchyLevelForAltitude(altM);

        const matchedPoly = this.findPolygonForCoordinate(lat, lon, targetLevel);
        this.highlightSelectedBoundary(matchedPoly, { lat, lon }, targetLevel);

        this.updateSelectionHeader(
          'RESOLVING...',
          '◈',
          `Resolving ${targetLevel.toUpperCase()}...`,
          `WGS84: [${lat.toFixed(3)}°, ${lon.toFixed(3)}°]`
        );

        const hierarchy = await resolveGeo(lat, lon, targetLevel);
        this.displayGeographicBreakdown(lat, lon, hierarchy, altM);

        const queryCity = (targetLevel === 'city' || targetLevel === 'county') ? (hierarchy.city || hierarchy.county) : undefined;
        const queryState = (targetLevel !== 'country') ? hierarchy.state : undefined;
        const queryCountry = hierarchy.country;

        const articles = await fetchNews(lat, lon, queryCity, queryState, queryCountry, targetLevel);
        if (articles.length > 0) {
          this.renderNewsArticles(articles);
        }
      }
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    // 2. DOUBLE CLICK: Precision pinpoint coordinate marker (blue dot)
    handler.setInputAction(async (click: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      const cartesian = this.viewer.camera.pickEllipsoid(
        click.position,
        this.viewer.scene.globe.ellipsoid
      );

      if (cartesian) {
        const cartographic = Cesium.Cartographic.fromCartesian(cartesian);
        const lat = Cesium.Math.toDegrees(cartographic.latitude);
        const lon = Cesium.Math.toDegrees(cartographic.longitude);
        this.placePinpointMarker(lat, lon);
      }
    }, Cesium.ScreenSpaceEventType.LEFT_DOUBLE_CLICK);
  }

  /**
   * Highlights the selected entity with a high-contrast HUD reticle ring.
   */
  private highlightSelectedEntity(entity: Cesium.Entity) {
    const oldReticle = this.reticleDataSource.entities.getById('selected_entity_reticle');
    if (oldReticle) {
      this.reticleDataSource.entities.remove(oldReticle);
    }

    if (!entity.position) return;

    this.reticleDataSource.entities.add({
      id: 'selected_entity_reticle',
      position: entity.position,
      point: {
        pixelSize: 32,
        color: Cesium.Color.fromCssColorString('#388bfd').withAlpha(0.20),
        outlineColor: Cesium.Color.fromCssColorString('#58a6ff'),
        outlineWidth: 2,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      },
    });
  }

  /**
   * Drops a precise pinpoint coordinate marker (blue dot) only on double-click.
   */
  private placePinpointMarker(lat: number, lon: number) {
    const existing = this.reticleDataSource.entities.getById('selected_location_marker');
    if (existing) {
      existing.position = new Cesium.ConstantPositionProperty(Cesium.Cartesian3.fromDegrees(lon, lat, 100));
    } else {
      this.reticleDataSource.entities.add({
        id: 'selected_location_marker',
        position: Cesium.Cartesian3.fromDegrees(lon, lat, 100),
        point: {
          pixelSize: 9,
          color: Cesium.Color.fromCssColorString('#388bfd'),
          outlineColor: Cesium.Color.fromCssColorString('#ffffff'),
          outlineWidth: 2,
          disableDepthTestDistance: Number.POSITIVE_INFINITY,
        },
        label: {
          text: ` [${lat.toFixed(4)}°, ${lon.toFixed(4)}°]`,
          font: '10px monospace',
          fillColor: Cesium.Color.fromCssColorString('#58a6ff'),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          pixelOffset: new Cesium.Cartesian2(12, -2),
          eyeOffset: new Cesium.Cartesian3(0, 0, -50),
          disableDepthTestDistance: Number.POSITIVE_INFINITY,
        },
      });
    }
  }

  /**
   * Highlights the selected boundary (County, City, State, or Country) with a glowing outline and soft fill.
   */
  private highlightSelectedBoundary(
    poly: GeoPolygon | null | undefined,
    fallbackCenter?: { lat: number; lon: number },
    _level?: string
  ) {
    const oldReticle = this.reticleDataSource.entities.getById('selected_entity_reticle');
    if (oldReticle) this.reticleDataSource.entities.remove(oldReticle);

    const oldGlow = this.selectionDataSource.entities.getById('selected_boundary_glow');
    if (oldGlow) this.selectionDataSource.entities.remove(oldGlow);
    const oldFill = this.selectionDataSource.entities.getById('selected_boundary_fill');
    if (oldFill) this.selectionDataSource.entities.remove(oldFill);

    if (poly && poly.rings && poly.rings.length > 0) {
      // Find ring that encloses fallbackCenter, or use largest ring
      let bestRing = poly.rings[0];
      if (fallbackCenter && poly.rings.length > 1) {
        for (const ring of poly.rings) {
          if (this.isPointInRing(fallbackCenter.lon, fallbackCenter.lat, ring as [number, number][])) {
            bestRing = ring;
            break;
          }
        }
      }

      const flatCoords: number[] = [];
      for (const pt of bestRing) {
        flatCoords.push(pt[0], pt[1]);
      }

      if (flatCoords.length >= 6) {
        this.selectionDataSource.entities.add({
          id: 'selected_boundary_fill',
          polygon: {
            hierarchy: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            material: Cesium.Color.fromCssColorString('#58a6ff').withAlpha(0.20),
            heightReference: Cesium.HeightReference.CLAMP_TO_GROUND,
          },
        });

        this.selectionDataSource.entities.add({
          id: 'selected_boundary_glow',
          polyline: {
            positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            width: 3.5,
            material: new Cesium.PolylineGlowMaterialProperty({
              glowPower: 0.35,
              color: Cesium.Color.fromCssColorString('#58a6ff'),
            }),
            clampToGround: true,
          },
        });
      }
    }
  }

  /**
   * Ray-casting point-in-polygon test for closed rings.
   */
  private isPointInRing(lon: number, lat: number, ring: [number, number][]): boolean {
    let inside = false;
    for (let i = 0, j = ring.length - 1; i < ring.length; j = i++) {
      const xi = ring[i][0];
      const yi = ring[i][1];
      const xj = ring[j][0];
      const yj = ring[j][1];
      const intersect = yi > lat !== yj > lat && lon < ((xj - xi) * (lat - yi)) / (yj - yi) + xi;
      if (intersect) inside = !inside;
    }
    return inside;
  }

  /**
   * Finds the best matching boundary polygon for a coordinate based on the current zoom level hierarchy.
   */
  private findPolygonForCoordinate(lat: number, lon: number, targetLevel: GeoHierarchyLevel): GeoPolygon | null {
    if (!this.boundaries) return null;

    const inPoly = (p: GeoPolygon) => {
      if (!p.rings || p.rings.length === 0) return false;
      if (lon < p.min_lon || lon > p.max_lon || lat < p.min_lat || lat > p.max_lat) return false;
      return p.rings.some(ring => this.isPointInRing(lon, lat, ring as [number, number][]));
    };

    // 1. City level
    if (targetLevel === 'city') {
      if (this.boundaries.cities) {
        for (const poly of this.boundaries.cities) {
          if (inPoly(poly)) {
            return poly;
          }
        }
      }
      // Check proximity to a bundled place (within 35 km)
      if (this.boundaries.places) {
        for (const pl of this.boundaries.places) {
          const dlat = (pl.lat - lat) * 111.0;
          const dlon = (pl.lon - lon) * 111.0 * Math.cos((lat * Math.PI) / 180.0);
          const distKm = Math.sqrt(dlat * dlat + dlon * dlon);
          if (distKm <= 35.0) {
            const matched = this.boundaries.cities?.find(c => c.name.toLowerCase() === pl.name.toLowerCase());
            if (matched) return matched;
          }
        }
      }
    }

    // 2. County level (or fallback from city)
    if (targetLevel === 'county' || targetLevel === 'city') {
      for (const poly of this.boundaries.counties) {
        if (inPoly(poly)) {
          return poly;
        }
      }
    }

    // 3. State level (or fallback from county/city)
    if (targetLevel === 'state' || targetLevel === 'county' || targetLevel === 'city') {
      for (const poly of this.boundaries.states) {
        if (inPoly(poly)) {
          return poly;
        }
      }
    }

    // 4. Country level (or fallback)
    for (const poly of this.boundaries.countries) {
      if (inPoly(poly)) {
        return poly;
      }
    }

    return null;
  }

  public renderNewsArticles(articles: NewsArticle[]) {
    const feed = document.getElementById('news-feed');
    if (!feed) return;

    // Clear and populate with fresh dispatches
    feed.innerHTML = '';

    for (const art of articles.slice(0, 6)) {
      const timeStr = new Date(art.published_at).toISOString().substring(11, 16);
      let tagClass = 'tag-system';
      let tagText = '[NEWS]';

      if (art.provenance) {
        switch (art.provenance.type) {
          case 'Direct':
            tagClass = 'tag-direct';
            tagText = `[Direct: ${art.provenance.entity}]`;
            break;
          case 'Regional':
            tagClass = 'tag-regional';
            tagText = `[Regional: ${art.provenance.entity}]`;
            break;
          case 'Statewide':
            tagClass = 'tag-statewide';
            tagText = `[Statewide: ${art.provenance.entity}]`;
            break;
          case 'National':
            tagClass = 'tag-system';
            tagText = `[National: ${art.provenance.entity}]`;
            break;
        }
      }

      const item = document.createElement('article');
      item.className = 'feed-item';
      item.innerHTML = `
        <span class="item-tag ${tagClass}">${tagText}</span>
        <span class="item-time">${timeStr}</span>
        <div class="item-title">
          <a href="${art.url}" target="_blank" rel="noopener noreferrer" class="feed-item-link">${art.title}</a>
        </div>
        <div class="item-source">Source: ${art.source} (Verified Attribution)</div>
      `;
      feed.appendChild(item);
    }
  }

  public appendNewsDispatch(obs: Observation) {
    const feed = document.getElementById('news-feed');
    if (!feed) return;

    const timeStr = new Date(obs.timestamp).toISOString().substring(11, 16);
    let tag = '[TELEMETRY]';
    let tagClass = 'tag-system';

    if (obs.layer === 'quake') {
      tag = '[SEISMIC]';
      tagClass = 'tag-direct';
    } else if (obs.layer === 'aircraft') {
      tag = '[ADS-B]';
      tagClass = 'tag-direct';
    } else if (obs.layer === 'satellite') {
      tag = '[ORBIT]';
      tagClass = 'tag-system';
    } else if (obs.layer === 'space_weather') {
      tag = '[SPACE-WX]';
      tagClass = 'tag-system';
    } else if (obs.layer === 'power_grid') {
      tag = '[POWER]';
      tagClass = 'tag-system';
    }

    let title = `${obs.layer.toUpperCase()}: ${obs.entity_id ?? 'Unknown'}`;
    if (obs.layer === 'quake') {
      title = `Magnitude ${obs.metric_value} detected (${obs.attributes?.place ?? 'Unknown'})`;
    } else if (obs.layer === 'aircraft') {
      const callsign = obs.attributes?.flight?.trim() || obs.entity_id;
      title = `Flight ${callsign} at ${Math.round(obs.altitude_m ?? 0)} m altitude`;
    } else if (obs.layer === 'satellite') {
      title = `Orbital Pass: ${obs.attributes?.name ?? 'Satellite'} at ${Math.round((obs.altitude_m ?? 0) / 1000)} km altitude`;
    } else if (obs.layer === 'space_weather') {
      title = `NOAA Planetary Kp Index: ${obs.metric_value?.toFixed(1)}`;
    } else if (obs.layer === 'power_grid') {
      title = `Grid Fuel Mix: ${obs.attributes?.renewable_total_perc ?? 0}% Renewable`;
    }

    const item = document.createElement('article');
    item.className = 'feed-item';
    item.innerHTML = `
      <span class="item-tag ${tagClass}">${tag}</span>
      <span class="item-time">${timeStr}</span>
      <div class="item-title">${title}</div>
      <div class="item-source">Source: ${obs.provider} (Verified Telemetry)</div>
    `;

    feed.prepend(item);
    // Keep max 5 feed items
    while (feed.children.length > 5) {
      feed.removeChild(feed.lastChild!);
    }
  }

  public setLayerVisibility(layer: string, visible: boolean) {
    if (layer === 'aircraft') {
      this.aircraftDataSource.show = visible;
    } else if (layer === 'ship') {
      this.shipDataSource.show = visible;
    } else if (layer === 'satellite') {
      this.satDataSource.show = visible;
    } else if (layer === 'quake') {
      this.quakeDataSource.show = visible;
    } else if (layer === 'weather') {
      this.weatherDataSource.show = visible;
    } else if (layer === 'wildfire') {
      this.wildfireDataSource.show = visible;
    } else if (layer === 'air_quality') {
      this.airQualityDataSource.show = visible;
    } else if (layer === 'hydro') {
      this.hydroDataSource.show = visible;
    } else if (layer === 'buoy') {
      this.buoyDataSource.show = visible;
    } else if (layer === 'cyclone') {
      this.cycloneDataSource.show = visible;
    } else if (layer === 'volcano') {
      this.volcanoDataSource.show = visible;
    } else if (layer === 'submarine_cable') {
      this.cableDataSource.show = visible;
    } else if (layer === 'radiation') {
      this.radiationDataSource.show = visible;
    } else if (layer === 'aurora') {
      this.auroraDataSource.show = visible;
    } else if (layer === 'disaster') {
      this.disasterDataSource.show = visible;
    }
  }

  /**
   * Renders boundary outlines (countries, states, counties, municipal cities) and populated place markers.
   * Outlines are styled crisply and gated dynamically by camera altitude matching Google Maps hierarchical behavior.
   */
  public renderBoundaries(boundaries: BoundariesResponse) {
    this.boundaries = boundaries;
    this.countryBoundariesDataSource.entities.removeAll();
    this.stateBoundariesDataSource.entities.removeAll();
    this.countyBoundariesDataSource.entities.removeAll();
    this.cityBoundariesDataSource.entities.removeAll();
    this.placesDataSource.entities.removeAll();

    // 1. Countries (Level 0) - Crisp cyan #58a6ff outline at 0.70 alpha
    for (const poly of boundaries.countries) {
      if (!poly.rings || poly.rings.length === 0) continue;
      for (let r = 0; r < poly.rings.length; r++) {
        const ring = poly.rings[r];
        const flatCoords: number[] = [];
        for (const pt of ring) {
          flatCoords.push(pt[0], pt[1]);
        }
        if (flatCoords.length < 4) continue;

        this.countryBoundariesDataSource.entities.add({
          id: `boundary_country_${poly.id}_${r}`,
          name: poly.name,
          polyline: {
            positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            width: 2.0,
            material: new Cesium.ColorMaterialProperty(
              Cesium.Color.fromCssColorString('#58a6ff').withAlpha(0.70)
            ),
            clampToGround: true,
          },
          properties: new Cesium.PropertyBag({
            isBoundary: true,
            boundaryLevel: 'country',
            polygon: poly,
            name: poly.name,
          }),
        });
      }
    }

    // 2. States / Provinces (Level 1) - Light blue #79c0ff outline at 0.65 alpha
    for (const poly of boundaries.states) {
      if (!poly.rings || poly.rings.length === 0) continue;
      for (let r = 0; r < poly.rings.length; r++) {
        const ring = poly.rings[r];
        const flatCoords: number[] = [];
        for (const pt of ring) {
          flatCoords.push(pt[0], pt[1]);
        }
        if (flatCoords.length < 4) continue;

        this.stateBoundariesDataSource.entities.add({
          id: `boundary_state_${poly.id}_${r}`,
          name: poly.name,
          polyline: {
            positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            width: 1.8,
            material: new Cesium.ColorMaterialProperty(
              Cesium.Color.fromCssColorString('#79c0ff').withAlpha(0.65)
            ),
            clampToGround: true,
          },
          properties: new Cesium.PropertyBag({
            isBoundary: true,
            boundaryLevel: 'state',
            polygon: poly,
            name: poly.name,
          }),
        });
      }
    }

    // 3. Counties / Districts (Level 2) - Soft blue-gray #a5d6ff outline at 0.60 alpha
    for (const poly of boundaries.counties) {
      if (!poly.rings || poly.rings.length === 0) continue;
      for (let r = 0; r < poly.rings.length; r++) {
        const ring = poly.rings[r];
        const flatCoords: number[] = [];
        for (const pt of ring) {
          flatCoords.push(pt[0], pt[1]);
        }
        if (flatCoords.length < 4) continue;

        this.countyBoundariesDataSource.entities.add({
          id: `boundary_county_${poly.id}_${r}`,
          name: poly.name,
          polyline: {
            positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            width: 1.5,
            material: new Cesium.ColorMaterialProperty(
              Cesium.Color.fromCssColorString('#a5d6ff').withAlpha(0.60)
            ),
            clampToGround: true,
          },
          properties: new Cesium.PropertyBag({
            isBoundary: true,
            boundaryLevel: 'county',
            polygon: poly,
            name: poly.name,
          }),
        });
      }
    }

    // 4. Municipal Cities (Level 3) - Bright emerald / teal #56d364 outline at 0.70 alpha
    for (const poly of boundaries.cities || []) {
      if (!poly.rings || poly.rings.length === 0) continue;
      for (let r = 0; r < poly.rings.length; r++) {
        const ring = poly.rings[r];
        const flatCoords: number[] = [];
        for (const pt of ring) {
          flatCoords.push(pt[0], pt[1]);
        }
        if (flatCoords.length < 4) continue;

        this.cityBoundariesDataSource.entities.add({
          id: `boundary_city_${poly.id}_${r}`,
          name: poly.name,
          polyline: {
            positions: Cesium.Cartesian3.fromDegreesArray(flatCoords),
            width: 1.8,
            material: new Cesium.ColorMaterialProperty(
              Cesium.Color.fromCssColorString('#56d364').withAlpha(0.70)
            ),
            clampToGround: true,
          },
          properties: new Cesium.PropertyBag({
            isBoundary: true,
            boundaryLevel: 'city',
            polygon: poly,
            name: poly.name,
          }),
        });
      }
    }

    // 5. Populated Places / Cities (Level 3) - Monospace label + dot with distance display condition
    for (const place of boundaries.places) {
      let maxDistance = 500_000;
      let labelSize = '11px';
      if (place.population >= 1_000_000) {
        maxDistance = 4_500_000;
        labelSize = '12px';
      } else if (place.population >= 100_000) {
        maxDistance = 1_500_000;
        labelSize = '11px';
      }

      this.placesDataSource.entities.add({
        id: `place_${place.name.replace(/\s+/g, '_')}_${place.country_code}`,
        name: place.name,
        position: Cesium.Cartesian3.fromDegrees(place.lon, place.lat, 100),
        point: {
          pixelSize: place.population >= 1_000_000 ? 5 : 4,
          color: Cesium.Color.fromCssColorString('#c9d1d9').withAlpha(0.75),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 1.5,
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, maxDistance),
        },
        label: {
          text: place.name,
          font: `${labelSize} "SF Mono", "Segoe UI Mono", monospace`,
          style: Cesium.LabelStyle.FILL_AND_OUTLINE,
          fillColor: Cesium.Color.fromCssColorString('#e6edf3').withAlpha(0.9),
          outlineColor: Cesium.Color.fromCssColorString('#0d1117'),
          outlineWidth: 2,
          verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
          pixelOffset: new Cesium.Cartesian2(0, -6),
          distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, maxDistance),
          eyeOffset: new Cesium.Cartesian3(0, 0, -40),
        },
        properties: new Cesium.PropertyBag({
          isPlace: true,
          placeName: place.name,
          countryCode: place.country_code,
          stateCode: place.state_code,
          lat: place.lat,
          lon: place.lon,
          population: place.population,
        }),
      });
    }
  }
}
