//! angleseek: a static-shooter analysis tool for the SOTM solver.
//!
//! Sweeps a range of distances × hood angles and visualizes the required
//! flywheel muzzle velocity. Lets you see how the shooter behaves across
//! all ranges for each candidate hood angle, so you can pick the angle
//! that gives the flattest / most consistent velocity profile.

use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Points};

use sotm_rt::solve_muzzle_velocity;

// ----------------------------------------------------------------------------
// Input parameters
// ----------------------------------------------------------------------------

/// User-supplied sweep configuration. All distances in meters, angles in degrees.
#[derive(Clone, Debug)]
struct Params {
    /// Distance from the goal at which the sweep starts (meters).
    dist_min: f64,
    /// Distance from the goal at which the sweep ends (meters).
    dist_max: f64,
    /// Step size between distances (meters).
    dist_step: f64,
    /// Lowest hood angle to test (degrees).
    angle_min: f64,
    /// Highest hood angle to test (degrees).
    angle_max: f64,
    /// Step size between hood angles (degrees).
    angle_step: f64,
    /// Height of the goal relative to the muzzle (meters).
    goal_height: f64,
    /// Velocity ceiling (m/s). Solutions above this are treated as unsolvable.
    max_velocity: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            dist_min: 0.0,
            dist_max: 10.0,
            dist_step: 0.01,
            angle_min: 30.0,
            angle_max: 70.0,
            angle_step: 0.1,
            goal_height: 1.0,
            max_velocity: 30.0,
        }
    }
}

// ----------------------------------------------------------------------------
// Computed dataset
// ----------------------------------------------------------------------------

/// One cell of the (distance × angle) sweep grid.
#[derive(Clone, Copy, Debug)]
struct Cell {
    distance: f64,
    velocity: Option<f64>,
}

/// Per-angle column of the sweep grid plus summary statistics.
#[derive(Clone, Debug)]
struct AngleColumn {
    angle_deg: f64,
    cells: Vec<Cell>,
    /// Mean of the solvable velocities. `None` if no cells solved.
    avg_velocity: Option<f64>,
    /// max - min over the solvable velocities. `None` if <2 cells solved.
    velocity_spread: Option<f64>,
    /// min solvable velocity (for reference).
    min_velocity: Option<f64>,
    /// max solvable velocity (for reference).
    max_velocity: Option<f64>,
    /// Number of solvable cells.
    solvable_count: usize,
    /// Largest distance (m) from the origin with no solution. `None` if every
    /// distance solved. Useful for seeing how close the shooter can get before
    /// the chosen hood angle stops working.
    max_unsolvable_distance: Option<f64>,
}

/// Full sweep result: one column per hood angle.
#[derive(Clone, Debug)]
struct Dataset {
    params: Params,
    columns: Vec<AngleColumn>,
}

impl Dataset {
    /// Run the sweep. This is the only place that calls the SOTM solver.
    ///
    /// `progress` is updated as each angle column completes so the UI can
    /// report how far along the background computation is.
    fn compute(params: &Params, progress: Option<&Progress>) -> Self {
        let distances = generate_range(params.dist_min, params.dist_max, params.dist_step);
        let angles = generate_range(params.angle_min, params.angle_max, params.angle_step);

        let total_angles = angles.len();
        let columns: Vec<AngleColumn> = angles
            .iter()
            .enumerate()
            .map(|(i, &angle_deg)| {
                if let Some(p) = progress {
                    p.set(i, total_angles);
                }
                let cells: Vec<Cell> = distances
                    .iter()
                    .map(|&distance| Cell {
                        distance,
                        velocity: solve_muzzle_velocity(
                            distance,
                            params.goal_height,
                            angle_deg,
                            params.max_velocity,
                        ),
                    })
                    .collect();

                let solved: Vec<f64> = cells.iter().filter_map(|c| c.velocity).collect();
                let solvable_count = solved.len();

                let avg_velocity = if solvable_count > 0 {
                    Some(solved.iter().sum::<f64>() / solvable_count as f64)
                } else {
                    None
                };

                let min_velocity = solved.iter().copied().fold(None, |acc, v| match acc {
                    None => Some(v),
                    Some(m) => Some(m.min(v)),
                });
                let max_velocity = solved.iter().copied().fold(None, |acc, v| match acc {
                    None => Some(v),
                    Some(m) => Some(m.max(v)),
                });

                let velocity_spread = match (min_velocity, max_velocity) {
                    (Some(lo), Some(hi)) => Some(hi - lo),
                    _ => None,
                };

                // Largest unsolvable distance. The sweep is ordered by
                // increasing distance, so the last unsolvable cell is the max.
                let max_unsolvable_distance = cells
                    .iter()
                    .rev()
                    .find(|c| c.velocity.is_none())
                    .map(|c| c.distance);

                AngleColumn {
                    angle_deg,
                    cells,
                    avg_velocity,
                    velocity_spread,
                    min_velocity,
                    max_velocity,
                    solvable_count,
                    max_unsolvable_distance,
                }
            })
            .collect();

        Self {
            params: params.clone(),
            columns,
        }
    }
}

// ----------------------------------------------------------------------------
// Background computation plumbing
// ----------------------------------------------------------------------------

/// Shared progress counter for a running sweep.
#[derive(Clone)]
struct Progress {
    inner: Arc<Mutex<(usize, usize)>>,
}

impl Progress {
    fn new() -> Self {
        Self { inner: Arc::new(Mutex::new((0, 0))) }
    }
    fn set(&self, done: usize, total: usize) {
        if let Ok(mut g) = self.inner.lock() {
            *g = (done, total);
        }
    }
    fn get(&self) -> (usize, usize) {
        self.inner.lock().map(|g| *g).unwrap_or((0, 0))
    }
}

/// Handle to a background sweep. Holds the worker thread and the shared slot
/// the worker will write the finished `Dataset` into.
struct ComputeHandle {
    join: Option<JoinHandle<Option<Dataset>>>,
    result: Arc<Mutex<Option<Dataset>>>,
    progress: Progress,
}

impl ComputeHandle {
    /// Spawn the sweep on a background thread.
    fn spawn(params: Params) -> Self {
        let result: Arc<Mutex<Option<Dataset>>> = Arc::new(Mutex::new(None));
        let progress = Progress::new();
        let result_for_thread = result.clone();
        let progress_for_thread = progress.clone();

        let join = thread::spawn(move || {
            let ds = Dataset::compute(&params, Some(&progress_for_thread));
            if let Ok(mut g) = result_for_thread.lock() {
                *g = Some(ds);
            }
            result_for_thread.lock().ok().and_then(|g| g.clone())
        });

        Self { join: Some(join), result, progress }
    }

    /// Non-blocking check: returns the dataset if the worker is done.
    fn try_take(&self) -> Option<Dataset> {
        self.result.lock().ok().and_then(|mut g| g.take())
    }

    fn progress(&self) -> (usize, usize) {
        self.progress.get()
    }
}

impl Drop for ComputeHandle {
    fn drop(&mut self) {
        // If the worker is still running when the app is closing, wait for it
        // so we don't leave a detached thread touching shared state.
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Inclusive range generator that steps by `step` and is robust to float drift.
fn generate_range(start: f64, end: f64, step: f64) -> Vec<f64> {
    if step <= 0.0 || end < start {
        return vec![start];
    }
    let n = ((end - start) / step).round() as i64;
    (0..=n).map(|i| start + i as f64 * step).collect()
}

// ----------------------------------------------------------------------------
// UI
// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Screen {
    Input,
    Results,
}

#[derive(Clone, Debug, PartialEq)]
enum ResultsTab {
    Overview,
    PerAngle,
}

struct App {
    screen: Screen,
    params: Params,
    /// Scratch buffers for the text fields so the user can type freely.
    input_text: ParamsText,
    dataset: Option<Arc<Dataset>>,
    /// Index into `dataset.columns` for the currently selected hood angle.
    selected_angle_idx: usize,
    results_tab: ResultsTab,
    last_error: Option<String>,
    /// Active background computation, if any.
    compute: Option<ComputeHandle>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Input,
            params: Params::default(),
            input_text: ParamsText::from_params(&Params::default()),
            dataset: None,
            selected_angle_idx: 0,
            results_tab: ResultsTab::Overview,
            last_error: None,
            compute: None,
        }
    }
}

/// String-backed mirror of `Params` for egui text inputs.
#[derive(Clone, Debug)]
struct ParamsText {
    dist_min: String,
    dist_max: String,
    dist_step: String,
    angle_min: String,
    angle_max: String,
    angle_step: String,
    goal_height: String,
    max_velocity: String,
}

impl ParamsText {
    fn from_params(p: &Params) -> Self {
        Self {
            dist_min: p.dist_min.to_string(),
            dist_max: p.dist_max.to_string(),
            dist_step: p.dist_step.to_string(),
            angle_min: p.angle_min.to_string(),
            angle_max: p.angle_max.to_string(),
            angle_step: p.angle_step.to_string(),
            goal_height: p.goal_height.to_string(),
            max_velocity: p.max_velocity.to_string(),
        }
    }
}

impl App {
    fn parse_params(&self) -> Result<Params, String> {
        let parse = |s: &str, name: &str| -> Result<f64, String> {
            s.trim()
                .parse::<f64>()
                .map_err(|_| format!("Invalid value for {name}: {s:?}"))
        };

        let dist_min = parse(&self.input_text.dist_min, "distance min")?;
        let dist_max = parse(&self.input_text.dist_max, "distance max")?;
        let dist_step = parse(&self.input_text.dist_step, "distance step")?;
        let angle_min = parse(&self.input_text.angle_min, "angle min")?;
        let angle_max = parse(&self.input_text.angle_max, "angle max")?;
        let angle_step = parse(&self.input_text.angle_step, "angle step")?;
        let goal_height = parse(&self.input_text.goal_height, "goal height")?;
        let max_velocity = parse(&self.input_text.max_velocity, "max velocity")?;

        if dist_max < dist_min {
            return Err("Distance max must be >= distance min".into());
        }
        if angle_max < angle_min {
            return Err("Angle max must be >= angle min".into());
        }
        if dist_step <= 0.0 {
            return Err("Distance step must be > 0".into());
        }
        if angle_step <= 0.0 {
            return Err("Angle step must be > 0".into());
        }
        if angle_min <= 0.0 || angle_max >= 90.0 {
            return Err("Hood angle must be strictly between 0 and 90 degrees".into());
        }
        if max_velocity <= 0.0 {
            return Err("Max velocity must be > 0".into());
        }

        Ok(Params {
            dist_min,
            dist_max,
            dist_step,
            angle_min,
            angle_max,
            angle_step,
            goal_height,
            max_velocity,
        })
    }

    fn start_compute(&mut self) {
        match self.parse_params() {
            Ok(params) => {
                self.params = params.clone();
                self.last_error = None;
                self.compute = Some(ComputeHandle::spawn(params));
            }
            Err(e) => {
                self.last_error = Some(e);
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll the background compute, if one is running.
        if let Some(handle) = &self.compute {
            if let Some(ds) = handle.try_take() {
                self.dataset = Some(Arc::new(ds));
                self.selected_angle_idx = 0;
                self.screen = Screen::Results;
                self.compute = None;
            } else {
                // Keep repainting so the progress bar updates and we notice
                // when the worker finishes.
                ctx.request_repaint();
            }
        }

        match self.screen {
            Screen::Input => self.render_input(ctx),
            Screen::Results => self.render_results(ctx),
        }
    }
}

// ----------------------------------------------------------------------------
// Input screen
// ----------------------------------------------------------------------------

impl App {
    fn render_input(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("angleseek — SOTM hood-angle analysis");
            ui.add_space(8.0);
            ui.label(
                "Sweep the SOTM solver across a range of distances and hood angles \
                 to see how the required flywheel velocity behaves. Pick the hood \
                 angle that gives the flattest / most consistent velocity profile.",
            );
            ui.add_space(12.0);

            egui::Grid::new("params_grid")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Distance min (m)");
                    ui.text_edit_singleline(&mut self.input_text.dist_min);
                    ui.end_row();

                    ui.label("Distance max (m)");
                    ui.text_edit_singleline(&mut self.input_text.dist_max);
                    ui.end_row();

                    ui.label("Distance step (m)");
                    ui.text_edit_singleline(&mut self.input_text.dist_step);
                    ui.end_row();

                    ui.label("Angle min (deg)");
                    ui.text_edit_singleline(&mut self.input_text.angle_min);
                    ui.end_row();

                    ui.label("Angle max (deg)");
                    ui.text_edit_singleline(&mut self.input_text.angle_max);
                    ui.end_row();

                    ui.label("Angle step (deg)");
                    ui.text_edit_singleline(&mut self.input_text.angle_step);
                    ui.end_row();

                    ui.label("Goal height (m)");
                    ui.text_edit_singleline(&mut self.input_text.goal_height);
                    ui.end_row();

                    ui.label("Max velocity (m/s)");
                    ui.text_edit_singleline(&mut self.input_text.max_velocity);
                    ui.end_row();
                });

            ui.add_space(12.0);

            let computing = self.compute.is_some();
            if ui
                .add_enabled(!computing, egui::Button::new("Compute"))
                .clicked()
            {
                self.start_compute();
            }

            if let Some(handle) = &self.compute {
                ui.add_space(8.0);
                let (done, total) = handle.progress();
                let frac = if total > 0 {
                    done as f32 / total as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(frac)
                        .text(format!("Computing… {done}/{total} angles")),
                );
            }

            if let Some(err) = &self.last_error {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("Error: {err}"));
            }
        });
    }
}

// ----------------------------------------------------------------------------
// Results screen
// ----------------------------------------------------------------------------

impl App {
    fn render_results(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("results_top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("← Back").clicked() {
                    self.screen = Screen::Input;
                }
                ui.separator();
                ui.selectable_value(&mut self.results_tab, ResultsTab::Overview, "Overview");
                ui.selectable_value(&mut self.results_tab, ResultsTab::PerAngle, "Per-angle");
            });
        });

        let dataset = match &self.dataset {
            Some(d) => d.clone(),
            None => {
                egui::CentralPanel::default().show(ctx, |ui| ui.label("No dataset."));
                return;
            }
        };

        match self.results_tab {
            ResultsTab::Overview => self.render_overview(ctx, &dataset),
            ResultsTab::PerAngle => self.render_per_angle(ctx, &dataset),
        }
    }

    fn render_overview(&self, ctx: &egui::Context, dataset: &Dataset) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Overview");
            ui.label(format!(
                "Sweep: {}–{} m ({} m step) × {}–{}° ({}° step), goal height {} m, max velocity {} m/s",
                dataset.params.dist_min,
                dataset.params.dist_max,
                dataset.params.dist_step,
                dataset.params.angle_min,
                dataset.params.angle_max,
                dataset.params.angle_step,
                dataset.params.goal_height,
                dataset.params.max_velocity,
            ));
            ui.add_space(12.0);

            // ---- Graph 1: velocity spread (max - min) per hood angle ----
            ui.heading("Velocity spread per hood angle");
            ui.label("Difference between the highest and lowest solvable velocity at each hood angle.");
            self.plot_spread(ui, dataset);
            ui.add_space(16.0);

            // ---- Graph 2: average velocity per hood angle ----
            ui.heading("Average velocity per hood angle");
            ui.label("Mean solvable muzzle velocity at each hood angle.");
            self.plot_avg(ui, dataset);
            ui.add_space(16.0);

            // ---- Graph 3: max unsolvable distance per hood angle ----
            ui.heading("Max unsolvable distance per hood angle");
            ui.label(
                "Largest distance from the origin with no solution at each hood angle. \
                 Shows how close the shooter can get to the goal before the angle \
                 stops working. A point at the top of the plot means the entire \
                 range is solvable up to dist_max.",
            );
            self.plot_max_unsolvable(ui, dataset);
        });
    }

    fn plot_spread(&self, ui: &mut egui::Ui, dataset: &Dataset) {
        let points: PlotPoints = dataset
            .columns
            .iter()
            .filter_map(|c| c.velocity_spread.map(|v| [c.angle_deg, v]))
            .collect();
        let line = Line::new(points).color(egui::Color32::from_rgb(220, 120, 40));
        Plot::new("spread_plot")
            .x_axis_label("Hood angle (°)")
            .y_axis_label("Velocity spread (m/s)")
            .show(ui, |p| p.line(line));
    }

    fn plot_avg(&self, ui: &mut egui::Ui, dataset: &Dataset) {
        let points: PlotPoints = dataset
            .columns
            .iter()
            .filter_map(|c| c.avg_velocity.map(|v| [c.angle_deg, v]))
            .collect();
        let line = Line::new(points).color(egui::Color32::from_rgb(80, 160, 240));
        Plot::new("avg_plot")
            .x_axis_label("Hood angle (°)")
            .y_axis_label("Average velocity (m/s)")
            .show(ui, |p| p.line(line));
    }

    fn plot_max_unsolvable(&self, ui: &mut egui::Ui, dataset: &Dataset) {
        // For angles where every distance solved, plot dist_max as the cap so
        // the graph still shows a meaningful "fully solvable" line.
        let dist_max = dataset.params.dist_max;
        let points: PlotPoints = dataset
            .columns
            .iter()
            .map(|c| {
                [
                    c.angle_deg,
                    c.max_unsolvable_distance.unwrap_or(dist_max),
                ]
            })
            .collect();
        let line = Line::new(points).color(egui::Color32::from_rgb(180, 80, 220));
        Plot::new("max_unsolvable_plot")
            .x_axis_label("Hood angle (°)")
            .y_axis_label("Max unsolvable distance (m)")
            .show(ui, |p| p.line(line));
    }

    fn render_per_angle(&mut self, ctx: &egui::Context, dataset: &Dataset) {
        // Angle selector (top panel).
        egui::TopBottomPanel::top("angle_selector").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Hood angle:");
                let slider_range = if dataset.columns.is_empty() {
                    0.0..=0.0
                } else {
                    dataset.columns.first().unwrap().angle_deg
                        ..=dataset.columns.last().unwrap().angle_deg
                };
                let mut selected_angle = dataset
                    .columns
                    .get(self.selected_angle_idx)
                    .map(|c| c.angle_deg)
                    .unwrap_or(0.0);
                if ui
                    .add(
                        egui::Slider::new(&mut selected_angle, slider_range)
                            .step_by(dataset.params.angle_step)
                            .text("°"),
                    )
                    .changed()
                {
                    // Snap to the nearest column.
                    self.selected_angle_idx = dataset
                        .columns
                        .iter()
                        .enumerate()
                        .min_by(|(_, a), (_, b)| {
                            (a.angle_deg - selected_angle)
                                .abs()
                                .partial_cmp(&(b.angle_deg - selected_angle).abs())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
                ui.separator();
                if let Some(col) = dataset.columns.get(self.selected_angle_idx) {
                    ui.monospace(format!(
                        "Selected: {:.3}°   ({} / {} distances solvable)",
                        col.angle_deg,
                        col.solvable_count,
                        col.cells.len()
                    ));
                }
            });
        });

        let col = match dataset.columns.get(self.selected_angle_idx) {
            Some(c) => c.clone(),
            None => {
                egui::CentralPanel::default().show(ctx, |ui| ui.label("No column selected."));
                return;
            }
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(format!("Hood angle: {:.3}°", col.angle_deg));
            ui.add_space(8.0);

            // ---- Cards ----
            ui.horizontal(|ui| {
                self.card(ui, "Average velocity", col.avg_velocity, "m/s");
                self.card(ui, "Velocity spread", col.velocity_spread, "m/s");
                self.card(ui, "Min velocity", col.min_velocity, "m/s");
                self.card(ui, "Max velocity", col.max_velocity, "m/s");
                self.card(
                    ui,
                    "Max unsolvable dist",
                    col.max_unsolvable_distance,
                    "m",
                );
            });
            ui.add_space(12.0);

            // ---- Per-angle velocity-vs-distance graph ----
            ui.heading("Velocity vs distance");
            ui.label(
                "Solvable distances are drawn as a line; unsolvable distances \
                 are drawn as red markers on the x-axis to show the no-solution region.",
            );
            self.plot_velocity_vs_distance(ui, &col);
        });
    }

    fn card(&self, ui: &mut egui::Ui, title: &str, value: Option<f64>, unit: &str) {
        let frame = egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
            .inner_margin(egui::Margin::same(10.0));
        frame.show(ui, |ui| {
            ui.set_min_width(140.0);
            ui.label(egui::RichText::new(title).small().color(egui::Color32::from_gray(180)));
            ui.add_space(4.0);
            match value {
                Some(v) => ui.label(
                    egui::RichText::new(format!("{v:.3} {unit}"))
                        .strong()
                        .size(20.0),
                ),
                None => ui.label(
                    egui::RichText::new("N/A")
                        .strong()
                        .size(20.0)
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                ),
            }
        });
    }

    fn plot_velocity_vs_distance(&self, ui: &mut egui::Ui, col: &AngleColumn) {
        // Solvable points as a connected line.
        let solvable: Vec<[f64; 2]> = col
            .cells
            .iter()
            .filter_map(|c| c.velocity.map(|v| [c.distance, v]))
            .collect();

        // Unsolvables as red markers at y = 0 (so they show up on the x-axis).
        let unsolvable: Vec<[f64; 2]> = col
            .cells
            .iter()
            .filter(|c| c.velocity.is_none())
            .map(|c| [c.distance, 0.0])
            .collect();

        let line = Line::new(PlotPoints::from(solvable))
            .color(egui::Color32::from_rgb(80, 200, 120));
        let markers = Points::new(PlotPoints::from(unsolvable))
            .color(egui::Color32::from_rgb(220, 60, 60))
            .radius(2.0);

        Plot::new("velocity_vs_distance_plot")
            .x_axis_label("Distance from goal (m)")
            .y_axis_label("Muzzle velocity (m/s)")
            .show(ui, |p| {
                p.line(line);
                p.points(markers);
            });
    }
}

// ----------------------------------------------------------------------------
// Entry point
// ----------------------------------------------------------------------------

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_title("angleseek"),
        ..Default::default()
    };
    eframe::run_native(
        "angleseek",
        opts,
        Box::new(|_cc| Ok(Box::<App>::default())),
    )
}
