#!/usr/bin/env python3
"""Generate minimal synthetic scenarios for ABM testing."""

import json
import numpy as np
from pathlib import Path
import struct

def create_synthetic_scenario(scenario_dir: Path, config: dict):
    """Create a minimal synthetic scenario."""
    scenario_dir.mkdir(parents=True, exist_ok=True)

    width_m = config['world_size_m'][0]
    height_m = config['world_size_m'][1]
    rows, cols = config['fire_grid_size']
    cellsize = width_m / cols

    # Simple DEM: slight slope, some variation
    dem_data = np.ones(rows * cols, dtype=np.float64) * 100.0
    for i in range(rows):
        for j in range(cols):
            dem_data[i * cols + j] += (i + j) * 0.1 + np.random.randn() * 5

    # Fuel: mostly grass (class 1), some heavier fuel (class 5)
    fuel_data = np.ones(rows * cols, dtype=np.int32)
    # Add some heavier fuel patches
    for _ in range(5):
        ci, cj = np.random.randint(0, cols), np.random.randint(0, rows)
        for di in range(-10, 10):
            for dj in range(-10, 10):
                i, j = ci + di, cj + dj
                if 0 <= i < rows and 0 <= j < cols:
                    fuel_data[i * cols + j] = 5

    # Render terrain (2x resolution of fire grid)
    render_rows, render_cols = rows * 2, cols * 2
    render_terrain = np.zeros(render_rows * render_cols, dtype=np.float32)
    for i in range(render_rows):
        for j in range(render_cols):
            fi, fj = i // 2, j // 2
            render_terrain[i * render_cols + j] = float(dem_data[fi * cols + fj])

    # Buildings: 2-3 clusters
    buildings = []
    for cluster in range(2):
        center_x = np.random.uniform(width_m * 0.2, width_m * 0.8)
        center_y = np.random.uniform(height_m * 0.2, height_m * 0.8)
        for _ in range(np.random.randint(2, 5)):
            x = center_x + np.random.randn() * 100
            y = center_y + np.random.randn() * 100
            x = max(0, min(width_m - 20, x))
            y = max(0, min(height_m - 20, y))
            buildings.append({
                'id': len(buildings) + 1,
                'kind': 'residential',
                'name': None,
                'centroid': [x, y],
                'ring': [
                    [x, y], [x + 20, y], [x + 20, y + 15], [x, y + 15]
                ]
            })

    # Roads: simple grid
    roads = []
    road_id = 1
    for x in np.arange(0, width_m, 200):
        roads.append({
            'id': road_id,
            'class': 'tertiary',
            'drivable': True,
            'track': False,
            'name': f'Road {road_id}',
            'oneway': False,
            'line': [[x, 0], [x, height_m]]
        })
        road_id += 1

    for y in np.arange(0, height_m, 200):
        roads.append({
            'id': road_id,
            'class': 'tertiary',
            'drivable': True,
            'track': False,
            'name': f'Road {road_id}',
            'oneway': False,
            'line': [[0, y], [width_m, y]]
        })
        road_id += 1

    # Water sources: 1-2 locations
    water = []
    for i in range(2):
        water.append({
            'id': i + 1,
            'kind': 'hydrant',
            'pos': [
                np.random.uniform(width_m * 0.1, width_m * 0.9),
                np.random.uniform(height_m * 0.1, height_m * 0.9)
            ]
        })

    # Population: 5-20 people total
    num_people = config.get('people_count', 10)
    households = []
    people = []
    for h_id in range(min(3, num_people)):
        dwelling_id = h_id % len(buildings)
        x, y = buildings[dwelling_id]['centroid']
        households.append({
            'dwelling_id': dwelling_id,
            'size': min(5, num_people - len(people)),
            'vehicles': np.random.randint(0, 2),
            'intent': np.random.choice(['leave_early', 'wait_and_see', 'stay_defend']),
            'risk_perception': np.random.uniform(0.3, 0.8),
            'trust_authority': np.random.uniform(0.3, 0.8),
            'prep_time_min': np.random.uniform(5, 30),
            'warning_channel': np.random.choice(['mobile_alert', 'siren', 'self_observed', 'none']),
            'at_home': True,
            'needs_assistance': False
        })
        for p in range(households[-1]['size']):
            people.append({
                'household_id': h_id,
                'name': f'P{len(people)+1}',
                'age': np.random.randint(5, 85),
                'needs_assistance': np.random.random() < 0.1,
                'at_home': True
            })

    # OSM vectors
    vectors = {
        'world_size_m': [width_m, height_m],
        'fire_grid': {
            'rows': rows,
            'cols': cols,
            'cellsize': cellsize
        },
        'buildings': buildings,
        'roads': roads,
        'water': water
    }

    # Population
    population = {
        'synthetic': True,
        'seed': 42,
        'dwellings': [
            {
                'building_id': b['id'],
                'people': np.random.randint(1, 4)
            }
            for b in buildings
        ],
        'households': households,
        'people': people
    }

    # Write binary files
    dem_data.astype('<f8').tofile(scenario_dir / 'dem.f64')
    fuel_data.astype('<i4').tofile(scenario_dir / 'fuel.i32')
    render_terrain.astype('<f4').tofile(scenario_dir / 'render_terrain.f32')

    # Write JSON files
    (scenario_dir / 'osm.json').write_text(json.dumps(vectors, indent=2))
    (scenario_dir / 'population.json').write_text(json.dumps(population, indent=2))

    # Render terrain metadata
    render_meta = {
        'rows': render_rows,
        'cols': render_cols,
        'posting_m': cellsize / 2,
        'world_size_m': [width_m, height_m],
        'elev_min': float(dem_data.min()),
        'elev_max': float(dem_data.max())
    }
    (scenario_dir / 'render_terrain.json').write_text(json.dumps(render_meta, indent=2))

    print(f"Created {scenario_dir.name}: {len(buildings)} buildings, {len(people)} people")

def main():
    data_dir = Path(__file__).parent.parent / 'data' / 'scenarios'

    # Small test scenario: 256x256 grid, 1-2 buildings, 5 people
    create_synthetic_scenario(data_dir / 'test_small', {
        'world_size_m': [5120.0, 5120.0],  # 256x256 @ 20m
        'fire_grid_size': [256, 256],
        'people_count': 5,
    })

    # Medium test: 256x256, 3-5 buildings, 15 people
    create_synthetic_scenario(data_dir / 'test_medium', {
        'world_size_m': [5120.0, 5120.0],
        'fire_grid_size': [256, 256],
        'people_count': 15,
    })

    # Urban test: 256x256, dense buildings, 20+ people
    create_synthetic_scenario(data_dir / 'test_urban', {
        'world_size_m': [5120.0, 5120.0],
        'fire_grid_size': [256, 256],
        'people_count': 20,
    })

if __name__ == '__main__':
    main()
