// NETRUNNER — scheletro di gioco a turni su griglia (Rust + macroquad)
//
// Obiettivo: raccogli tutti i DATA (floppy gialli) e raggiungi il nodo EXIT
// (verde) prima che il TRACE arrivi al massimo. Ogni furto genera un nuovo ICE.
// Gli ICE (sagome con visore rosso) ti inseguono: si muovono ogni `ice_every`
// tue mosse, valore che dipende dal livello (vedi LEVELS).
//
// Comandi: frecce per muoverti, SPAZIO per l'EMP (congela gli ICE),
//          A per ricominciare a fine partita, F11 schermo intero, ESC esci.

use macroquad::audio::*;
use macroquad::prelude::*;

// --- Costanti di configurazione -------------------------------------------

const TILE: f32 = 48.0; // dimensione di una cella in pixel
const HUD_H: f32 = 64.0; // altezza della barra in alto
const EMP_TURNS: u32 = 6; // per quante mosse gli ICE restano congelati

// Tutto ciò che distingue un livello dall'altro.
struct LevelDef {
    name: &'static str,           // nome mostrato a schermo
    map: &'static [&'static str], // # muro . pavimento P giocatore D dati E uscita I ICE
    trace_max: u32,               // mosse prima di essere tracciato
    ice_every: u32,               // gli ICE si muovono ogni N mosse
    emp_charges: u32,             // cariche EMP a disposizione
}

// L'elenco dei livelli, in ordine.
const LEVELS: &[LevelDef] = &[
    LevelDef {
        name: "MERCATO NERO",
        map: &[
            "############",
            "#P...#....D#",
            "#.##.#.##..#",
            "#..D...#..I#",
            "##.###.#.###",
            "#....I.....#",
            "#.##.####.##",
            "#D........E#",
            "############",
        ],
        trace_max: 60,
        ice_every: 2,
        emp_charges: 2,
    },
    LevelDef {
        name: "CLINICA FANTASMA",
        map: &[
            "############",
            "#P..#...D..#",
            "#.#.#.###.##",
            "#.#...#I...#",
            "#.###.#.##.#",
            "#D..#...#..#",
            "###.#.#.#.##",
            "#I....#..DE#",
            "############",
        ],
        trace_max: 50,
        ice_every: 2,
        emp_charges: 2,
    },
    LevelDef {
        name: "BANCA ORBITALE",
        map: &[
            "############",
            "#P.#.....#D#",
            "#..#.###.#.#",
            "##...#D#...#",
            "#..#.#.#.#.#",
            "#.##...I.#.#",
            "#..I.#####.#",
            "#D#.......E#",
            "############",
        ],
        trace_max: 55,
        ice_every: 2,
        emp_charges: 2,
    },
    LevelDef {
        name: "TORRE KAZE-CORP",
        map: &[
            "############",
            "#D...#P...D#",
            "#.##.#.##..#",
            "#..I......##",
            "##.#.##.#..#",
            "#..#.#D.#I.#",
            "#.##.#.##..#",
            "#D..I.....E#",
            "############",
        ],
        trace_max: 52,
        ice_every: 2,
        emp_charges: 2,
    },
    LevelDef {
        name: "DATAFORTEZZA",
        map: &[
            "############",
            "#P..#..I..D#",
            "#.#.#.##.#.#",
            "#.#D..#..#.#",
            "#.###.#.##.#",
            "#.I...D.#..#",
            "#.#####.#I.#",
            "#D........E#",
            "############",
        ],
        trace_max: 47,
        ice_every: 2,
        emp_charges: 2,
    },
];

// Risoluzione "virtuale" del gioco: disegniamo sempre in queste coordinate,
// poi la camera le scala per riempire la finestra (qualunque sia la sua
// dimensione o lo zoom dello schermo).
// Tutti i livelli hanno la stessa dimensione: basta guardare il primo.
const VIRTUAL_W: f32 = LEVELS[0].map[0].len() as f32 * TILE;
const VIRTUAL_H: f32 = LEVELS[0].map.len() as f32 * TILE + HUD_H;

// Palette neon (r, g, b, alpha da 0.0 a 1.0)
const BG: Color = Color::new(0.04, 0.02, 0.08, 1.0);
const GRID_DIM: Color = Color::new(0.25, 0.12, 0.40, 1.0);
const NEON_CYAN: Color = Color::new(0.0, 0.95, 1.0, 1.0);
const NEON_PINK: Color = Color::new(1.0, 0.16, 0.60, 1.0);
const NEON_YELLOW: Color = Color::new(0.95, 1.0, 0.20, 1.0);
const NEON_GREEN: Color = Color::new(0.20, 1.0, 0.45, 1.0);
const ICE_RED: Color = Color::new(1.0, 0.15, 0.20, 1.0);
const ICE_FROZEN: Color = Color::new(0.65, 0.85, 1.0, 1.0); // azzurro ghiaccio

// --- Tipi di dato ---------------------------------------------------------

// `Copy` permette di copiare un Tile come un intero, senza problemi di ownership.
#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Floor,
    Wall,
    Data,
    Exit,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Playing,
    LevelClear, // livello superato, in attesa di passare al prossimo
    Won,        // tutti i livelli superati
    Lost,
}

struct Ice {
    x: i32,
    y: i32,
}

// Tutto lo stato del gioco vive qui dentro: niente variabili globali,
// niente riferimenti incrociati. È il modo più semplice per andare
// d'accordo con il borrow checker.
struct Game {
    level: usize, // indice del livello attuale in LEVELS (0 = primo)
    w: i32,
    h: i32,
    tiles: Vec<Tile>, // griglia "appiattita": indice = y * w + x
    px: i32,
    py: i32,
    ice: Vec<Ice>,
    data_left: u32,
    data_total: u32,
    trace: u32,
    emp_charges: u32,
    frozen: u32,
    alarm_at: f64, // istante dell'ultimo allarme, per l'effetto flash
    state: State,
    reason: &'static str, // perché hai perso (mostrato a schermo)
}

// --- Logica ---------------------------------------------------------------

impl Game {
    fn new(level: usize) -> Self {
        let def = &LEVELS[level]; // i dati del livello richiesto
        let h = def.map.len() as i32;
        let w = def.map[0].len() as i32;
        let mut tiles = Vec::with_capacity((w * h) as usize);
        let (mut px, mut py) = (0, 0);
        let mut ice = Vec::new();
        let mut data_left = 0;

        for (y, row) in def.map.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let (x, y) = (x as i32, y as i32);
                let tile = match c {
                    '#' => Tile::Wall,
                    'D' => {
                        data_left += 1;
                        Tile::Data
                    }
                    'E' => Tile::Exit,
                    'P' => {
                        px = x;
                        py = y;
                        Tile::Floor
                    }
                    'I' => {
                        ice.push(Ice { x, y });
                        Tile::Floor
                    }
                    _ => Tile::Floor,
                };
                tiles.push(tile);
            }
        }

        Self {
            level,
            w,
            h,
            tiles,
            px,
            py,
            ice,
            data_left,
            data_total: data_left,
            trace: 0,
            alarm_at: -10.0,
            emp_charges: def.emp_charges,
            frozen: 0, // a inizio partita gli ICE NON sono congelati
            state: State::Playing,
            reason: "",
        }
    }

    // Scorciatoia per leggere i dati del livello in corso.
    fn def(&self) -> &'static LevelDef {
        &LEVELS[self.level]
    }

    // Fuori dalla mappa = muro, così non serve controllare i bordi altrove.
    fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return Tile::Wall;
        }
        self.tiles[(y * self.w + x) as usize]
    }

    fn set_tile(&mut self, x: i32, y: i32, t: Tile) {
        let i = (y * self.w + x) as usize;
        self.tiles[i] = t;
    }

    // Restituisce la direzione premuta in questo frame, se c'è.
    fn read_input() -> Option<(i32, i32)> {
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) {
            Some((0, -1))
        } else if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) {
            Some((0, 1))
        } else if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::A) {
            Some((-1, 0))
        } else if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) {
            Some((1, 0))
        } else {
            None
        }
    }

    fn update(&mut self) {
        // A partita finita aspettiamo solo il tasto A.
        if self.state != State::Playing {
            if is_key_pressed(KeyCode::A) {
                *self = match self.state {
                    State::LevelClear => Game::new(self.level + 1), // prossimo livello
                    State::Lost => Game::new(self.level),           // ritenta questo
                    State::Won | State::Playing => Game::new(0),    // da capo
                };
            }
            return;
        }

        // EMP: Spazio congela tutti gli ICE per EMP_TURNS mosse.
        if is_key_pressed(KeyCode::Space) && self.emp_charges > 0 && self.frozen == 0 {
            self.emp_charges -= 1;
            self.frozen = EMP_TURNS;
            self.alarm_at = get_time(); // riusa il flash come effetto visivo
            return; // l'EMP non consuma la mossa
        }

        // Gioco a turni: se non premi nulla, il mondo resta fermo.
        let Some((dx, dy)) = Self::read_input() else {
            return;
        };

        let (nx, ny) = (self.px + dx, self.py + dy);
        let target = self.tile(nx, ny);

        if target == Tile::Wall {
            return; // mossa non valida: non consuma il turno
        }

        // Mossa valida: il giocatore si sposta e il trace avanza.
        self.px = nx;
        self.py = ny;
        self.trace += 1;

        match target {
            Tile::Data => {
                self.set_tile(nx, ny, Tile::Floor);
                self.data_left -= 1;
                // Ogni furto fa scattare l'allarme: nuovo ICE in arrivo.
                self.spawn_ice();
                self.alarm_at = get_time();
            }
            Tile::Exit if self.data_left == 0 => {
                // C'è un livello dopo questo?
                if self.level + 1 < LEVELS.len() {
                    self.state = State::LevelClear;
                } else {
                    self.state = State::Won;
                }
                return;
            }
            _ => {}
        }

        // Sei entrato tu dentro un ICE? (se è congelato, puoi passarci sopra)
        if self.frozen == 0 && self.caught() {
            self.lose("sei entrato in un ICE");
            return;
        }

        // Turno degli ICE.
        if self.frozen > 0 {
            // Congelati dall'EMP: restano fermi, e il contatore scende di uno.
            self.frozen -= 1;
        } else if self.trace % self.def().ice_every == 0 {
            // Comportamento normale: si muovono una volta ogni ICE_EVERY mosse.
            self.move_ice();
            if self.caught() {
                self.lose("un ICE ti ha raggiunto");
                return;
            }
        }

        if self.trace >= self.def().trace_max {
            self.lose("trace completato");
        }
    }

    fn lose(&mut self, reason: &'static str) {
        self.state = State::Lost;
        self.reason = reason;
    }

    // Fa comparire un ICE sulla cella di pavimento più lontana dal giocatore
    // (distanza Manhattan), così non ti nasce addosso.
    fn spawn_ice(&mut self) {
        let mut best: Option<(i32, i32)> = None;
        let mut best_d = -1;

        for y in 0..self.h {
            for x in 0..self.w {
                if self.tile(x, y) != Tile::Floor {
                    continue;
                }
                if self.ice.iter().any(|i| i.x == x && i.y == y) {
                    continue; // cella già occupata da un altro ICE
                }
                let d = (x - self.px).abs() + (y - self.py).abs();
                if d > best_d {
                    best_d = d;
                    best = Some((x, y));
                }
            }
        }

        if let Some((x, y)) = best {
            self.ice.push(Ice { x, y });
        }
    }

    fn caught(&self) -> bool {
        self.ice.iter().any(|i| i.x == self.px && i.y == self.py)
    }

    // NOTA SUL BORROW CHECKER — il pattern da ricordare:
    // non possiamo modificare self.ice (prestito mutabile) mentre chiamiamo
    // self.step_toward (prestito immutabile di tutto self).
    // Soluzione: prima CALCOLIAMO le nuove posizioni leggendo soltanto,
    // poi le SCRIVIAMO in un secondo passaggio.
    fn move_ice(&mut self) {
        let targets: Vec<(i32, i32)> = self
            .ice
            .iter()
            .map(|i| self.step_toward(i.x, i.y))
            .collect();

        for (ice, (x, y)) in self.ice.iter_mut().zip(targets) {
            ice.x = x;
            ice.y = y;
        }
    }

    // IA "avida": l'ICE fa il passo che lo avvicina di più al giocatore.
    // È volutamente stupida: si incastra dietro i muri, e tu puoi sfruttarlo.
    fn step_toward(&self, x: i32, y: i32) -> (i32, i32) {
        let dist = |ax: i32, ay: i32| (ax - self.px).abs() + (ay - self.py).abs();
        let mut best = (x, y);
        let mut best_d = dist(x, y);

        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let (nx, ny) = (x + dx, y + dy);
            if matches!(self.tile(nx, ny), Tile::Wall | Tile::Exit) {
                continue;
            }
            let d = dist(nx, ny);
            if d < best_d {
                best_d = d;
                best = (nx, ny);
            }
        }
        best
    }

    // --- Disegno ----------------------------------------------------------

    fn draw(&self, spr: &Sprites) {
        fit_camera();
        draw_rectangle(0.0, 0.0, VIRTUAL_W, VIRTUAL_H, BG);
        let t = get_time() as f32; // tempo in secondi, per le animazioni
        let pulse = (t * 4.0).sin() * 0.5 + 0.5; // oscilla tra 0 e 1

        // Griglia
        for y in 0..self.h {
            for x in 0..self.w {
                let sx = x as f32 * TILE;
                let sy = y as f32 * TILE + HUD_H;
                let cx = sx + TILE / 2.0;
                let cy = sy + TILE / 2.0;

                match self.tile(x, y) {
                    Tile::Floor => draw_circle(cx, cy, 2.0, GRID_DIM),
                    Tile::Wall => {
                        draw_rectangle(sx, sy, TILE, TILE, Color::new(0.0, 0.15, 0.20, 1.0));
                        draw_rectangle_lines(
                            sx + 2.0,
                            sy + 2.0,
                            TILE - 4.0,
                            TILE - 4.0,
                            2.0,
                            NEON_CYAN,
                        );
                    }
                    Tile::Data => {
                        draw_circle(cx, cy, 2.0, GRID_DIM);
                        draw_floppy(cx, cy, NEON_YELLOW, pulse);
                    }
                    Tile::Exit => {
                        let c = if self.data_left == 0 {
                            NEON_GREEN
                        } else {
                            GRID_DIM
                        };
                        draw_rectangle_lines(sx + 6.0, sy + 6.0, TILE - 12.0, TILE - 12.0, 3.0, c);
                        draw_text("EX", sx + 13.0, sy + 31.0, 22.0, c);
                    }
                }
            }
        }

        // Fotogramma dell'animazione: alterna 0 e 1 due volte al secondo.
        let frame = (get_time() * 2.0) as usize % 2;

        // ICE: sprite animato; congelato = fermo sul primo fotogramma e tinta azzurra
        let (ice_frame, ice_tint) = if self.frozen > 0 {
            (0, ICE_FROZEN)
        } else {
            (frame, WHITE)
        };
        // Alone dietro la sagoma scura: rosso se attivo, azzurro se congelato.
        let aura = if self.frozen > 0 {
            Color::new(0.4, 0.7, 1.0, 0.30)
        } else {
            Color::new(1.0, 0.1, 0.2, 0.25)
        };
        for i in &self.ice {
            let x = i.x as f32 * TILE;
            let y = i.y as f32 * TILE + HUD_H;
            draw_rectangle(x + 4.0, y + 4.0, TILE - 8.0, TILE - 8.0, aura);
            draw_sprite(&spr.ice, ice_frame, x, y, ice_tint);
        }

        // Giocatore: alone magenta + sprite animato
        let gx = self.px as f32 * TILE;
        let gy = self.py as f32 * TILE + HUD_H;
        let glow = Color::new(NEON_PINK.r, NEON_PINK.g, NEON_PINK.b, 0.18);
        draw_rectangle(gx + 4.0, gy + 4.0, TILE - 8.0, TILE - 8.0, glow);
        draw_sprite(&spr.player, frame, gx, gy, WHITE);

        self.draw_alarm();
        self.draw_hud();
        self.draw_overlay();
        draw_scanlines();
    }

    fn draw_hud(&self) {
        let w = VIRTUAL_W;
        let got = self.data_total - self.data_left;
        draw_text(
            &format!("DATA {}/{}", got, self.data_total),
            16.0,
            28.0,
            26.0,
            NEON_YELLOW,
        );
        let (exit_txt, exit_col) = if self.data_left == 0 {
            ("EXIT OPEN >> raggiungi EX", NEON_GREEN)
        } else {
            ("EXIT LOCKED", Color::new(0.55, 0.35, 0.75, 1.0))
        };
        draw_text(exit_txt, 16.0, 52.0, 20.0, exit_col);

        // Cariche EMP rimaste, al centro dell'HUD
        let emp_color = if self.emp_charges > 0 {
            NEON_CYAN
        } else {
            GRID_DIM
        };
        draw_text(
            &format!("EMP x{}", self.emp_charges),
            220.0,
            28.0,
            22.0,
            emp_color,
        );

        // Se gli ICE sono congelati, mostra quante mosse mancano
        if self.frozen > 0 {
            draw_text(
                &format!("FROZEN {}", self.frozen),
                220.0,
                52.0,
                20.0,
                ICE_FROZEN,
            );
        }

        // Barra del trace: da ciano a rosso man mano che si riempie
        let ratio = self.trace as f32 / self.def().trace_max as f32;
        let bar_x = w - 236.0;
        let bar_color = Color::new(ratio, 0.95 * (1.0 - ratio), 1.0 - ratio * 0.8, 1.0);
        draw_text("TRACE", bar_x, 28.0, 22.0, NEON_CYAN);
        // Numero del livello, accanto alla scritta TRACE
        let node = format!("NODO {}/{}", self.level + 1, LEVELS.len());
        draw_text(&node, bar_x + 130.0, 28.0, 20.0, NEON_PINK);
        draw_rectangle_lines(bar_x, 36.0, 220.0, 14.0, 2.0, NEON_CYAN);
        draw_rectangle(bar_x + 2.0, 38.0, 216.0 * ratio, 10.0, bar_color);

        draw_line(0.0, HUD_H - 2.0, w, HUD_H - 2.0, 2.0, NEON_PINK);
    }

    // Flash rosso che svanisce in 0.6 secondi dopo un allarme.
    fn draw_alarm(&self) {
        let elapsed = (get_time() - self.alarm_at) as f32;
        if elapsed > 0.6 {
            return;
        }
        let alpha = 0.35 * (1.0 - elapsed / 0.6);
        draw_rectangle(
            0.0,
            HUD_H,
            VIRTUAL_W,
            VIRTUAL_H - HUD_H,
            Color::new(1.0, 0.0, 0.1, alpha),
        );
        let msg = "! ALARM: NEW ICE DEPLOYED !";
        let size = measure_text(msg, None, 28, 1.0);
        draw_text(
            msg,
            (VIRTUAL_W - size.width) / 2.0,
            HUD_H + 40.0,
            28.0,
            ICE_RED,
        );
    }

    fn draw_overlay(&self) {
        let (title, color, hint) = match self.state {
            State::Playing => return,
            State::LevelClear => ("NODO VIOLATO", NEON_GREEN, "premi A per il prossimo nodo"),
            State::Won => ("EXFIL COMPLETE", NEON_GREEN, "premi A per ricominciare"),
            State::Lost => ("FLATLINED", ICE_RED, "premi A per riconnetterti"),
        };
        let (w, h) = (VIRTUAL_W, VIRTUAL_H);
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));

        // Centra il testo misurandone la larghezza
        let size = measure_text(title, None, 48, 1.0);
        draw_text(title, (w - size.width) / 2.0, h / 2.0, 48.0, color);

        // Sottotitolo: il motivo della sconfitta, o il nome del prossimo nodo
        let subtitle = match self.state {
            State::Lost => self.reason.to_string(),
            State::LevelClear => format!("prossimo: {}", LEVELS[self.level + 1].name),
            _ => String::new(),
        };
        if !subtitle.is_empty() {
            let size = measure_text(&subtitle, None, 26, 1.0);
            draw_text(
                &subtitle,
                (w - size.width) / 2.0,
                h / 2.0 + 36.0,
                26.0,
                NEON_YELLOW,
            );
        }

        let size = measure_text(hint, None, 24, 1.0);
        draw_text(
            hint,
            (w - size.width) / 2.0,
            h / 2.0 + 72.0,
            24.0,
            NEON_CYAN,
        );
    }
}

// Imposta una camera che mappa l'area virtuale VIRTUAL_W x VIRTUAL_H
// sulla finestra reale, mantenendo le proporzioni (bande nere ai lati).
fn fit_camera() {
    // Usiamo solo il RAPPORTO larghezza/altezza della finestra: così le unità
    // di misura (pixel logici o fisici, zoom dello schermo) non contano più.
    let screen_ratio = screen_width() / screen_height();
    let game_ratio = VIRTUAL_W / VIRTUAL_H;

    // Allarghiamo l'area inquadrata dalla camera oltre il gioco, in modo che
    // abbia le stesse proporzioni della finestra: il gioco resta al centro.
    let view = if screen_ratio > game_ratio {
        // finestra più larga del gioco: bande ai lati
        let view_w = VIRTUAL_H * screen_ratio;
        Rect::new(-(view_w - VIRTUAL_W) / 2.0, 0.0, view_w, VIRTUAL_H)
    } else {
        // finestra più alta del gioco: bande sopra e sotto
        let view_h = VIRTUAL_W / screen_ratio;
        Rect::new(0.0, -(view_h - VIRTUAL_H) / 2.0, VIRTUAL_W, view_h)
    };

    clear_background(BLACK); // colore delle bande
    let mut cam = Camera2D::from_display_rect(view);
    cam.zoom.y = -cam.zoom.y; // asse y verso il basso, come lo schermo
    set_camera(&cam);
}

fn draw_floppy(cx: f32, cy: f32, color: Color, pulse: f32) {
    let s = 12.0; // metà del lato
    let dark = Color::new(0.10, 0.05, 0.15, 1.0);
    // alone pulsante
    let glow = Color::new(color.r, color.g, color.b, 0.15 + pulse * 0.2);
    draw_rectangle(
        cx - s - 3.0,
        cy - s - 3.0,
        s * 2.0 + 6.0,
        s * 2.0 + 6.0,
        glow,
    );
    // corpo
    draw_rectangle(cx - s, cy - s, s * 2.0, s * 2.0, color);
    // angolo smussato in alto a destra
    draw_triangle(
        vec2(cx + s - 5.0, cy - s),
        vec2(cx + s, cy - s),
        vec2(cx + s, cy - s + 5.0),
        BG,
    );
    // sportellino metallico con finestrella
    draw_rectangle(cx - 6.0, cy - s, 12.0, 8.0, dark);
    draw_rectangle(cx + 1.0, cy - s + 1.5, 3.0, 5.0, color);
    // etichetta con due righe "scritte"
    draw_rectangle(cx - 8.0, cy + 1.0, 16.0, 10.0, dark);
    draw_rectangle(cx - 6.0, cy + 3.0, 12.0, 1.5, color);
    draw_rectangle(cx - 6.0, cy + 6.5, 9.0, 1.5, color);
}

// Effetto monitor CRT: righe scure orizzontali sopra tutto.
fn draw_scanlines() {
    let (w, h) = (VIRTUAL_W, VIRTUAL_H);
    let line = Color::new(0.0, 0.0, 0.0, 0.22);
    let mut y = 0.0;
    while y < h {
        draw_line(0.0, y, w, y, 1.0, line);
        y += 3.0;
    }
}

// --- Grafica ----------------------------------------------------------------

// Le immagini del gioco. Ogni file contiene 2 fotogrammi quadrati affiancati.
struct Sprites {
    player: Texture2D,
    ice: Texture2D,
}

impl Sprites {
    async fn load() -> Self {
        let player = load_texture("assets/player.png").await.unwrap();
        let ice = load_texture("assets/ice.png").await.unwrap();
        // Pixel art: ingrandendo, i pixel restano netti invece di sfumare.
        player.set_filter(FilterMode::Nearest);
        ice.set_filter(FilterMode::Nearest);
        Self { player, ice }
    }
}

// Disegna il fotogramma `frame` di uno sprite, ingrandito a una cella intera.
// `tint` colora l'immagine: WHITE = colori originali.
fn draw_sprite(tex: &Texture2D, frame: usize, x: f32, y: f32, tint: Color) {
    // Fotogrammi quadrati: il lato è l'altezza dell'immagine (16, 24, 32...).
    let size = tex.height();
    draw_texture_ex(
        tex,
        x,
        y,
        tint,
        DrawTextureParams {
            dest_size: Some(vec2(TILE, TILE)),
            source: Some(Rect::new(frame as f32 * size, 0.0, size, size)),
            ..Default::default()
        },
    );
}

// --- Audio ------------------------------------------------------------------

// Tutti i suoni del gioco, caricati una volta sola all'avvio.
struct Sounds {
    pickup: Sound,
    alarm: Sound,
    emp: Sound,
    flatline: Sound,
    clear: Sound,
    music: Sound,
}

impl Sounds {
    // `async` perché leggere file dal disco richiede tempo: con `.await`
    // aspettiamo che ogni file sia caricato prima di passare al successivo.
    async fn load() -> Self {
        Self {
            pickup: load_sound("assets/pickup.wav").await.unwrap(),
            alarm: load_sound("assets/alarm.wav").await.unwrap(),
            emp: load_sound("assets/emp.wav").await.unwrap(),
            flatline: load_sound("assets/flatline.wav").await.unwrap(),
            clear: load_sound("assets/clear.wav").await.unwrap(),
            music: load_sound("assets/music.wav").await.unwrap(),
        }
    }
}

// --- Finestra e game loop ---------------------------------------------------

fn window_conf() -> Conf {
    Conf {
        window_title: "NETRUNNER".to_owned(),
        // Finestra iniziale a 2x la risoluzione virtuale; puoi ridimensionarla
        // o massimizzarla e il gioco si adatta.
        window_width: (VIRTUAL_W * 2.0) as i32,
        window_height: (VIRTUAL_H * 2.0) as i32,
        window_resizable: true,
        high_dpi: true,
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // In sviluppo, lavora sempre dalla cartella del progetto (dove sta assets/),
    // da qualunque posto venga lanciato il gioco (terminale, Sublime...).
    #[cfg(debug_assertions)]
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();
    let sfx = Sounds::load().await;
    // Musica di sottofondo: in loop, a volume basso per non coprire gli effetti.
    play_sound(
        &sfx.music,
        PlaySoundParams {
            looped: true,
            volume: 0.35,
        },
    );
    let spr = Sprites::load().await;

    let mut game = Game::new(0); // si parte dal primo livello
    let mut fullscreen = true;
    loop {
        if is_key_pressed(KeyCode::Escape) {
            break; // ESC chiude il gioco
        }
        if is_key_pressed(KeyCode::F11) {
            fullscreen = !fullscreen; // ! inverte il booleano
            set_fullscreen(fullscreen);
        }

        // "Fotografia" di alcuni valori PRIMA dell'aggiornamento...
        let before_state = game.state;
        let before_data = game.data_left;
        let before_emp = game.emp_charges;

        game.update(); // 1. input + logica

        // ...e confronto DOPO: ogni differenza è un evento da far suonare.
        if game.state == before_state && game.data_left < before_data {
            play_sound_once(&sfx.pickup);
            play_sound_once(&sfx.alarm);
        }
        if game.emp_charges < before_emp {
            play_sound_once(&sfx.emp);
        }
        if game.state != before_state {
            match game.state {
                State::Lost => play_sound_once(&sfx.flatline),
                State::LevelClear | State::Won => play_sound_once(&sfx.clear),
                State::Playing => {} // si è appena (ri)partiti: nessun suono
            }
        }

        game.draw(&spr); //   2. disegno
        next_frame().await // 3. mostra il frame
    }
}
