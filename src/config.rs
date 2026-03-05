use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "openclaw-matrix", version, about = "Jack into the Matrix. A terminal client for openclaw.")]
pub struct Config {
    /// Rain speed
    #[arg(long, default_value = "medium", value_parser = parse_speed)]
    pub speed: Speed,

    /// Column density
    #[arg(long, default_value = "medium", value_parser = parse_density)]
    pub density: Density,

    /// Rain color (green, blue, red, cyan, purple, white, or #RRGGBB)
    #[arg(long, default_value = "green")]
    pub color: String,

    /// Target frame rate
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u16).range(10..=120))]
    pub fps: u16,

    /// Character set
    #[arg(long, default_value = "default", value_parser = parse_charset)]
    pub charset: Charset,

    /// RNG seed for reproducible output
    #[arg(long)]
    pub seed: Option<u64>,

    /// Screensaver mode (no input box, no gateway connection)
    #[arg(long)]
    pub no_input: bool,

    /// Disable flash effects (accessibility)
    #[arg(long)]
    pub reduce_motion: bool,

    // --- Gateway flags ---

    /// Override gateway WebSocket URL (default: read from ~/.openclaw/openclaw.json)
    #[arg(long)]
    pub gateway_url: Option<String>,

    /// Override gateway auth token
    #[arg(long)]
    pub token: Option<String>,

    /// Visual-only mode — do not connect to gateway
    #[arg(long)]
    pub offline: bool,
}

impl Config {
    /// Whether to attempt gateway connection
    pub fn should_connect(&self) -> bool {
        !self.offline && !self.no_input
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Speed {
    Slow,
    Medium,
    Fast,
}

#[derive(Debug, Clone, Copy)]
pub enum Density {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy)]
pub enum Charset {
    Default,
    Katakana,
    Ascii,
    Digits,
}

fn parse_speed(s: &str) -> Result<Speed, String> {
    match s.to_lowercase().as_str() {
        "slow" => Ok(Speed::Slow),
        "medium" => Ok(Speed::Medium),
        "fast" => Ok(Speed::Fast),
        _ => Err(format!("Invalid speed: {s}. Use slow, medium, or fast.")),
    }
}

fn parse_density(s: &str) -> Result<Density, String> {
    match s.to_lowercase().as_str() {
        "low" => Ok(Density::Low),
        "medium" => Ok(Density::Medium),
        "high" => Ok(Density::High),
        _ => Err(format!("Invalid density: {s}. Use low, medium, or high.")),
    }
}

fn parse_charset(s: &str) -> Result<Charset, String> {
    match s.to_lowercase().as_str() {
        "default" => Ok(Charset::Default),
        "katakana" => Ok(Charset::Katakana),
        "ascii" => Ok(Charset::Ascii),
        "digits" => Ok(Charset::Digits),
        _ => Err(format!("Invalid charset: {s}. Use default, katakana, ascii, or digits.")),
    }
}

impl Speed {
    pub fn cells_per_tick(self) -> f32 {
        match self {
            Speed::Slow => 0.3,
            Speed::Medium => 0.6,
            Speed::Fast => 1.0,
        }
    }
}

impl Density {
    pub fn fraction(self) -> f32 {
        match self {
            Density::Low => 0.3,
            Density::Medium => 0.5,
            Density::High => 0.8,
        }
    }
}
