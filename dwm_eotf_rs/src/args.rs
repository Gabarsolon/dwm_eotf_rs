const DEFAULT_GAMMA: f32 = 2.2;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
const DEFAULT_WAIT_TIME: f32 = 5.0;

#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Optional brightness multiplier (factor for nits, e.g. 1.56, or 2.083 for 1000 nits)
    #[arg(long, default_value_t = DEFAULT_BRIGHTNESS)]
    pub brightness: f32,

    /// Target SDR brightness in nits (e.g. 1000). Automatically calculates multiplier based on display white level.
    #[arg(long)]
    pub nits: Option<f32>,

    /// Disable border & Mica alpha correction fix (borders remain visible by default)
    #[arg(long)]
    pub no_fix_borders: bool,

    /// Patch DWM and exit (disables tray mode)
    #[arg(short, long)]
    pub compatibility_mode: bool,

    /// Disable Multi-Plane Overlay (prevents windows from bypassing DWM)
    #[arg(short, long)]
    pub disable_mpo: bool,

    /// Exponent to use during EOTF patching
    #[arg(default_value_t = DEFAULT_GAMMA)]
    pub gamma: f32,

    /// Patch every shader that contains sRGB EOTF patterns
    #[arg(short, long)]
    pub ignore_whitelist: bool,

    /// Prevent automatic patching on app start (tray mode only)
    #[arg(short, long)]
    pub skip_patching: bool,

    /// Delay (in seconds) before patching on start (tray mode only)
    #[arg(short, long, default_value_t = DEFAULT_WAIT_TIME)]
    pub wait_time: f32,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Dumps DWM's original shaders as DXBC
    Dump {
        /// Prevents recursive dumping of sub-shaders
        #[arg(short, long)]
        big_shaders: bool,

        /// Target directory for dumped DXBC files
        #[arg(short, long, default_value = "shaders/dumped")]
        output_dir: std::path::PathBuf,
    },

    /// Restores original sRGB EOTF and enables Multi-Plane Overlay
    Restore,

    /// Creates a task ('dwm_eotf_rs') that runs the app on user logon
    Schedule,

    /// Removes the startup task from Task Scheduler
    Unschedule {
        /// Remove the task for all users
        #[arg(short, long)]
        all: bool,
    },
}

impl Args {
    pub fn effective_brightness(&self) -> f32 {
        if let Some(target_nits) = self.nits {
            let current_white_level = crate::registry::get_primary_sdr_white_level().unwrap_or(480.0);
            if current_white_level > 0.0 {
                let factor = target_nits / current_white_level;
                tracing::info!(
                    "Targeting {:.1} nits with SDR base white level {:.1} nits -> brightness multiplier {:.3}x",
                    target_nits, current_white_level, factor
                );
                return factor;
            }
        }
        self.brightness
    }

    pub fn fix_borders(&self) -> bool {
        !self.no_fix_borders
    }

    pub fn serialize_args(&self) -> String {
        let mut arguments = Vec::with_capacity(6);

        if self.ignore_whitelist {
            arguments.push("-i".to_string());
        }

        if self.skip_patching {
            arguments.push("-s".to_string());
        }

        if self.compatibility_mode {
            arguments.push("-c".to_string());
        }

        if self.disable_mpo {
            arguments.push("-d".to_string());
        }

        if self.wait_time != DEFAULT_WAIT_TIME {
            arguments.push(format!("-w {:.1}", self.wait_time));
        }

        if let Some(nits) = self.nits {
            arguments.push(format!("--nits {:.1}", nits));
        } else if self.brightness != DEFAULT_BRIGHTNESS {
            arguments.push(format!("--brightness {:.3}", self.brightness));
        }

        if self.no_fix_borders {
            arguments.push("--no-fix-borders".to_string());
        }

        arguments.push(format!("{:.3}", self.gamma));
        arguments.join(" ")
    }
}
