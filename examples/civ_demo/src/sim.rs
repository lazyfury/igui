//! The pure civilization simulation: a square grid of tiles, resources,
//! gathering, population and consumption.
//!
//! No rendering and no scene tree — the whole loop is deterministic and unit
//! tested on its own. The view in [`crate`] merely mirrors this state, and the
//! simulation tick is a node callback (`set_physics_process`).
//!
//! # The loop (one [`Sim::tick`])
//!
//! 1. **Gather** — every assigned worker yields its tile's resource (× a
//!    building bonus) into `food` / `wood` / `stone`.
//! 2. **Consume** — the population eats `FOOD_PER_POP` food each.
//! 3. **Grow / starve** — a positive food surplus fills `growth` and adds a
//!    citizen up to the housing cap; a food shortage costs a citizen.

/// Grid width in tiles.
pub const GRID_W: usize = 16;
/// Grid height in tiles.
pub const GRID_H: usize = 11;
/// Tile edge length in logical pixels (square grid).
pub const TILE: f32 = 32.0;
/// Population the settlement supports without any house.
pub const BASE_POP_CAP: u32 = 4;
/// Extra population each house supports.
pub const HOUSE_POP_CAP: u32 = 5;
/// Food one citizen eats per tick.
pub const FOOD_PER_POP: f32 = 0.35;
/// Food surplus (production − consumption) needed to grow one citizen.
pub const GROWTH_PER_POP: f32 = 2.0;

/// A gatherable resource.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resource {
    Food,
    Wood,
    Stone,
}

/// A tile's terrain, which decides what can be gathered and built.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Terrain {
    /// Yields food; farms.
    Grass,
    /// Yields wood; lumber camps.
    #[default]
    Forest,
    /// Yields stone; quarries.
    Mountain,
    /// Impassable and unbuildable.
    Water,
}

impl Terrain {
    /// The resource a worker gathers here (`None` for water).
    pub fn resource(self) -> Option<Resource> {
        match self {
            Terrain::Grass => Some(Resource::Food),
            Terrain::Forest => Some(Resource::Wood),
            Terrain::Mountain => Some(Resource::Stone),
            Terrain::Water => None,
        }
    }

    /// Resource per assigned worker per tick, before a building bonus.
    pub fn yield_per_worker(self) -> f32 {
        match self {
            Terrain::Grass => 0.9,
            Terrain::Forest => 0.7,
            Terrain::Mountain => 0.5,
            Terrain::Water => 0.0,
        }
    }

    /// Whether anything can be built here.
    pub fn buildable(self) -> bool {
        self != Terrain::Water
    }
}

/// A building placed on a tile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Building {
    /// Raises the population cap. No gathering.
    House,
    /// Boosts food gathering on its (grass) tile.
    Farm,
    /// Boosts wood gathering on its (forest) tile.
    LumberCamp,
    /// Boosts stone gathering on its (mountain) tile.
    Quarry,
}

impl Building {
    /// Every buildable kind, in menu order.
    pub const ALL: [Building; 4] = [
        Building::House,
        Building::Farm,
        Building::LumberCamp,
        Building::Quarry,
    ];

    /// A short display name.
    pub fn label(self) -> &'static str {
        match self {
            Building::House => "House",
            Building::Farm => "Farm",
            Building::LumberCamp => "Camp",
            Building::Quarry => "Quarry",
        }
    }

    /// Build cost as `(food, wood, stone)`.
    pub fn cost(self) -> (u32, u32, u32) {
        match self {
            Building::House => (0, 12, 0),
            Building::Farm => (0, 6, 0),
            Building::LumberCamp => (0, 6, 0),
            Building::Quarry => (0, 8, 4),
        }
    }

    /// Extra population cap this building provides.
    pub fn pop_cap(self) -> u32 {
        match self {
            Building::House => HOUSE_POP_CAP,
            _ => 0,
        }
    }

    /// Gathering multiplier this building gives its tile's terrain.
    pub fn gather_mult(self, terrain: Terrain) -> f32 {
        match (self, terrain) {
            (Building::Farm, Terrain::Grass)
            | (Building::LumberCamp, Terrain::Forest)
            | (Building::Quarry, Terrain::Mountain) => 1.6,
            _ => 1.0,
        }
    }

    /// Whether this building may be placed on `terrain`.
    pub fn matches(self, terrain: Terrain) -> bool {
        match self {
            Building::House => terrain.buildable(),
            Building::Farm => terrain == Terrain::Grass,
            Building::LumberCamp => terrain == Terrain::Forest,
            Building::Quarry => terrain == Terrain::Mountain,
        }
    }
}

/// One square of the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub building: Option<Building>,
    /// Workers assigned to gather here.
    pub workers: u32,
}

/// The whole civilization state.
#[derive(Clone, Debug)]
pub struct Sim {
    pub tiles: Vec<Tile>,
    pub food: f32,
    pub wood: f32,
    pub stone: f32,
    pub population: u32,
    /// Accumulated food surplus toward the next citizen.
    pub growth: f32,
    /// Number of ticks run (for display / tests).
    pub ticks: u64,
}

impl Default for Sim {
    fn default() -> Self {
        Self::new()
    }
}

impl Sim {
    /// A deterministic starting map with three citizens and a small stockpile.
    pub fn new() -> Self {
        let mut tiles = Vec::with_capacity(GRID_W * GRID_H);
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                tiles.push(Tile {
                    terrain: terrain_at(x, y),
                    building: None,
                    workers: 0,
                });
            }
        }
        let mut sim = Self {
            tiles,
            food: 20.0,
            wood: 25.0,
            stone: 0.0,
            population: 3,
            growth: 0.0,
            ticks: 0,
        };
        sim.assign_default_workers();
        sim
    }

    pub fn in_bounds(x: usize, y: usize) -> bool {
        x < GRID_W && y < GRID_H
    }

    pub fn index(x: usize, y: usize) -> usize {
        y * GRID_W + x
    }

    pub fn tile(&self, x: usize, y: usize) -> &Tile {
        &self.tiles[Self::index(x, y)]
    }

    /// Total workers currently assigned across the map.
    pub fn assigned_workers(&self) -> u32 {
        self.tiles.iter().map(|tile| tile.workers).sum()
    }

    /// Workers not yet assigned (population minus assignments).
    pub fn free_workers(&self) -> u32 {
        self.population.saturating_sub(self.assigned_workers())
    }

    /// Population supported by the base plus every house.
    pub fn pop_cap(&self) -> u32 {
        BASE_POP_CAP
            + self
                .tiles
                .iter()
                .filter_map(|tile| tile.building)
                .map(Building::pop_cap)
                .sum::<u32>()
    }

    /// Whether the stockpile covers a building's cost.
    pub fn can_afford(&self, building: Building) -> bool {
        let (food, wood, stone) = building.cost();
        self.food >= food as f32 && self.wood >= wood as f32 && self.stone >= stone as f32
    }

    /// Whether `building` may be placed at `(x, y)` right now.
    pub fn can_build(&self, x: usize, y: usize, building: Building) -> bool {
        if !Self::in_bounds(x, y) {
            return false;
        }
        let tile = self.tile(x, y);
        tile.building.is_none()
            && tile.terrain.buildable()
            && building.matches(tile.terrain)
            && self.can_afford(building)
    }

    /// Spends the cost and places `building`. Returns whether it happened.
    pub fn build(&mut self, x: usize, y: usize, building: Building) -> bool {
        if !self.can_build(x, y, building) {
            return false;
        }
        let (food, wood, stone) = building.cost();
        self.food -= food as f32;
        self.wood -= wood as f32;
        self.stone -= stone as f32;
        self.tiles[Self::index(x, y)].building = Some(building);
        true
    }

    /// Toggles one worker on `(x, y)`. Returns whether a worker is now here.
    ///
    /// Refuses water and refuses to assign beyond the free-worker count.
    pub fn toggle_worker(&mut self, x: usize, y: usize) -> bool {
        if !Self::in_bounds(x, y) {
            return false;
        }
        let free = self.free_workers();
        let tile = &mut self.tiles[Self::index(x, y)];
        if tile.workers > 0 {
            tile.workers -= 1;
            return false;
        }
        if tile.terrain.resource().is_none() || free == 0 {
            return false;
        }
        tile.workers += 1;
        true
    }

    /// Runs one simulation step (see the module docs).
    pub fn tick(&mut self) {
        self.ticks += 1;

        let mut income = [0.0f32; 3];
        for tile in &self.tiles {
            let Some(resource) = tile.terrain.resource() else {
                continue;
            };
            if tile.workers == 0 {
                continue;
            }
            let mult = tile
                .building
                .map_or(1.0, |building| building.gather_mult(tile.terrain));
            let amount = tile.workers as f32 * tile.terrain.yield_per_worker() * mult;
            match resource {
                Resource::Food => income[0] += amount,
                Resource::Wood => income[1] += amount,
                Resource::Stone => income[2] += amount,
            }
        }
        self.food += income[0];
        self.wood += income[1];
        self.stone += income[2];

        let need = self.population as f32 * FOOD_PER_POP;
        if self.food >= need {
            self.food -= need;
            let surplus = income[0] - need;
            if surplus > 0.0 && self.population < self.pop_cap() {
                self.growth += surplus;
                while self.growth >= GROWTH_PER_POP && self.population < self.pop_cap() {
                    self.growth -= GROWTH_PER_POP;
                    self.population += 1;
                }
            } else {
                self.growth = 0.0;
            }
        } else {
            self.food = 0.0;
            self.growth = 0.0;
            if self.population > 0 {
                self.population -= 1;
                self.trim_workers();
            }
        }
    }

    fn trim_workers(&mut self) {
        while self.assigned_workers() > self.population {
            let Some(tile) = self.tiles.iter_mut().find(|tile| tile.workers > 0) else {
                break;
            };
            tile.workers -= 1;
        }
    }

    fn assign_default_workers(&mut self) {
        let mut food = 0;
        let mut wood = 0;
        let mut assigned = 0;
        for tile in &mut self.tiles {
            if assigned >= self.population {
                break;
            }
            match tile.terrain {
                Terrain::Grass if food < 2 => {
                    tile.workers = 1;
                    food += 1;
                    assigned += 1;
                }
                Terrain::Forest if wood < 1 => {
                    tile.workers = 1;
                    wood += 1;
                    assigned += 1;
                }
                _ => {}
            }
        }
    }
}

/// Deterministic starting terrain: a pond, a mountain ridge and forest belts.
fn terrain_at(x: usize, y: usize) -> Terrain {
    if (2..=4).contains(&x) && (1..=2).contains(&y) {
        return Terrain::Water;
    }
    if (y == 3 && (9..=13).contains(&x)) || (x == 11 && (1..=3).contains(&y)) {
        return Terrain::Mountain;
    }
    if (y == 6 || y == 7) && (3..=12).contains(&x) {
        return Terrain::Forest;
    }
    if x <= 1 && y >= 6 {
        return Terrain::Forest;
    }
    Terrain::Grass
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_one_grass_worker() -> Sim {
        let mut sim = Sim::new();
        for tile in &mut sim.tiles {
            tile.workers = 0;
        }
        sim.population = 1;
        let grass = sim
            .tiles
            .iter_mut()
            .find(|tile| tile.terrain == Terrain::Grass)
            .expect("a grass tile");
        grass.workers = 1;
        sim
    }

    #[test]
    fn gathering_adds_resources() {
        let mut sim = with_one_grass_worker();
        let before = sim.food;
        sim.tick();
        // 0.9 gathered, 0.35 eaten.
        assert!((sim.food - (before + 0.9 - 0.35)).abs() < 1e-4);
        assert_eq!(sim.ticks, 1);
    }

    #[test]
    fn population_consumes_food() {
        let mut sim = Sim::new();
        for tile in &mut sim.tiles {
            tile.workers = 0;
        }
        sim.food = 10.0;
        sim.population = 4;
        sim.tick();
        // No income, so exactly the food need is consumed.
        assert!((sim.food - (10.0 - 4.0 * FOOD_PER_POP)).abs() < 1e-4);
    }

    #[test]
    fn a_food_surplus_grows_the_population_to_the_cap() {
        let mut sim = Sim::new();
        for tile in &mut sim.tiles {
            tile.workers = 0;
        }
        // Four food workers feed a fast-growing settlement up to the base cap.
        let mut assigned = 0;
        for tile in &mut sim.tiles {
            if assigned >= 4 {
                break;
            }
            if tile.terrain == Terrain::Grass {
                tile.workers = 1;
                assigned += 1;
            }
        }
        sim.population = 1;
        sim.food = 100.0;
        for _ in 0..40 {
            sim.tick();
        }
        assert_eq!(sim.population, BASE_POP_CAP);
    }

    #[test]
    fn a_house_raises_the_cap() {
        let mut sim = Sim::new();
        sim.wood = 100.0;
        let (x, y) = (0, 0);
        assert_eq!(sim.tile(x, y).terrain, Terrain::Grass);
        assert!(sim.build(x, y, Building::House));
        assert_eq!(sim.pop_cap(), BASE_POP_CAP + HOUSE_POP_CAP);
    }

    #[test]
    fn starvation_costs_a_citizen() {
        let mut sim = Sim::new();
        for tile in &mut sim.tiles {
            tile.workers = 0;
        }
        sim.food = 0.0;
        sim.population = 3;
        sim.tick();
        assert_eq!(sim.population, 2);
        assert_eq!(sim.food, 0.0);
    }

    #[test]
    fn building_spends_resources_and_occupies_the_tile() {
        let mut sim = Sim::new();
        sim.wood = 20.0;
        let (x, y) = (0, 0);
        assert!(sim.build(x, y, Building::Farm));
        assert_eq!(sim.wood, 14.0, "the farm costs 6 wood");
        assert_eq!(sim.tile(x, y).building, Some(Building::Farm));
        assert!(!sim.build(x, y, Building::Farm), "the tile is occupied");
    }

    #[test]
    fn a_building_needs_matching_terrain() {
        let mut sim = Sim::new();
        sim.wood = 100.0;
        sim.stone = 100.0;
        // Find a water tile; nothing can be built there.
        let water = sim
            .tiles
            .iter()
            .position(|tile| tile.terrain == Terrain::Water)
            .expect("water");
        let (x, y) = (water % GRID_W, water / GRID_W);
        assert!(!sim.can_build(x, y, Building::House));
        // A farm needs grass, not forest.
        let forest = sim
            .tiles
            .iter()
            .position(|tile| tile.terrain == Terrain::Forest)
            .expect("forest");
        let (fx, fy) = (forest % GRID_W, forest / GRID_W);
        assert!(!sim.can_build(fx, fy, Building::Farm));
        assert!(sim.can_build(fx, fy, Building::LumberCamp));
    }

    #[test]
    fn workers_are_capped_by_population() {
        let mut sim = Sim::new();
        for tile in &mut sim.tiles {
            tile.workers = 0;
        }
        sim.population = 2;
        let grass: Vec<(usize, usize)> = sim
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| tile.terrain == Terrain::Grass)
            .map(|(i, _)| (i % GRID_W, i / GRID_W))
            .collect();
        assert!(sim.toggle_worker(grass[0].0, grass[0].1));
        assert!(sim.toggle_worker(grass[1].0, grass[1].1));
        assert!(
            !sim.toggle_worker(grass[2].0, grass[2].1),
            "no free workers left"
        );
        assert_eq!(sim.assigned_workers(), 2);
        // Toggling an assigned tile releases the worker.
        assert!(!sim.toggle_worker(grass[0].0, grass[0].1));
        assert_eq!(sim.assigned_workers(), 1);
    }
}
