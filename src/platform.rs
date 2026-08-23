use clap::Args;

#[derive(Args, Default, Clone, Debug)]
pub struct PlatformFlags {
    /// Export for Windows
    #[arg(long)]
    pub windows: bool,

    /// Export for Linux
    #[arg(long)]
    pub linux: bool,

    /// Export for Web
    #[arg(long)]
    pub web: bool,

    /// Export for macOS
    #[arg(long)]
    pub macos: bool,

    /// Export for iOS
    #[arg(long)]
    pub ios: bool,

    /// Export for Android
    #[arg(long)]
    pub android: bool,

    /// Export for visionOS
    #[arg(long)]
    pub visionos: bool,
}

impl PlatformFlags {
    pub fn any(&self) -> bool {
        self.windows
            || self.linux
            || self.web
            || self.macos
            || self.ios
            || self.android
            || self.visionos
    }

    pub fn to_platforms(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.windows {
            v.push("windows".into());
        }
        if self.linux {
            v.push("linux".into());
        }
        if self.web {
            v.push("web".into());
        }
        if self.macos {
            v.push("macos".into());
        }
        if self.ios {
            v.push("ios".into());
        }
        if self.android {
            v.push("android".into());
        }
        if self.visionos {
            v.push("visionos".into());
        }
        v
    }
}
