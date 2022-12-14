#[macro_use]
extern crate objc;

use std::collections::HashMap;
use std::process::exit;

use objc::rc::StrongPtr;
use objc::runtime::{Class, Object};
use objc::Encode;
// use objc::found
use clap::{arg, command, value_parser, Arg, ArgAction, Command};
use cocoa::appkit::NSScreen;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSArray, NSDictionary, NSRect, NSString, NSURL};
use directories::{BaseDirs, ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::time::{Duration, SystemTime};

fn nsstring(s: &str) -> StrongPtr {
    unsafe { StrongPtr::new(NSString::alloc(nil).init_str(s)) }
}

enum NSImageScaling {
    NSImageScaleAxesIndependently = 1,
    NSImageScaleNone = 2,
    NSImageScaleProportionallyUpOrDown = 3,
}

struct NSColor {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

fn set_wallpaper(
    display_id: u64,
    wallpaper_path: &str,
    background_color: NSColor,
    scaling: NSImageScaling,
    allow_clipping: bool,
) {
    let screens = unsafe { NSScreen::screens(nil) };
    let screen_count = unsafe { NSArray::count(screens) };
    let mut screen_to_set: Option<*mut Object> = None;

    for i in 0..screen_count {
        let screen = unsafe { NSArray::objectAtIndex(screens, i) };
        let screen_description = unsafe { NSScreen::deviceDescription(screen) };
        let screen_id_obj =
            unsafe { NSDictionary::objectForKey_(screen_description, *nsstring("NSScreenNumber")) };
        let screen_id: u64 = unsafe { msg_send![screen_id_obj, unsignedIntValue] };
        if screen_id == display_id {
            screen_to_set = Some(screen);
            break;
        }
    }

    let screen_to_set = screen_to_set.unwrap();

    let workspace: id = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    let wallpaper_url: id = unsafe { NSURL::fileURLWithPath_(nil, *nsstring(wallpaper_path)) };

    let rgba_color: id = unsafe {
        msg_send![class!(NSColor), colorWithSRGBRed:background_color.r green:background_color.g blue:background_color.b alpha:background_color.a]
    };

    let options_keys = vec![
        "NSWorkspaceDesktopImageScalingKey",
        "NSWorkspaceDesktopImageAllowClippingKey",
        "NSWorkspaceDesktopImageFillColorKey",
    ];
    let options_values: Vec<*mut Object> = vec![
        unsafe { msg_send![class!(NSNumber), numberWithInteger: scaling] },
        unsafe { msg_send![class!(NSNumber), numberWithInteger:(allow_clipping as u8)] },
        rgba_color,
    ];

    let mkstr = |s| unsafe { NSString::alloc(nil).init_str(s) };
    let keys_raw_vec = options_keys
        .clone()
        .into_iter()
        .map(&mkstr)
        .collect::<Vec<_>>();

    let keys_array = unsafe { NSArray::arrayWithObjects(nil, &keys_raw_vec) };
    let objs_array = unsafe { NSArray::arrayWithObjects(nil, &options_values) };

    let dict = unsafe { NSDictionary::dictionaryWithObjects_forKeys_(nil, objs_array, keys_array) };

    let _: id = unsafe {
        msg_send![workspace, setDesktopImageURL:wallpaper_url forScreen:screen_to_set options: dict error: nil]
    };
}

#[derive(Serialize, Deserialize, Debug)]
enum ImageScaling {
    #[serde(rename = "fit")]
    Fit,
    #[serde(rename = "fill")]
    Fill,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ImageSource {
    id: u64,
    url: String,
    estimated_size: String,
    update_interval: u64,
    dimensions: (u64, u64),
    #[serde(default)]
    is_thumbnail: bool,
    default_scaling: ImageScaling,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SatelliteView {
    id: u64,
    name: String,
    image_sources: Vec<ImageSource>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Satellite {
    id: u64,
    name: String,
    views: Vec<SatelliteView>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SatelliteConfig {
    // version: String,
    dns_http_probe_override: Vec<String>,
    satellites: Vec<Satellite>,
}

const SATELLITE_CONFIG_FILE: &'static str =
    "https://spaceeye-satellite-configs.s3.us-east-2.amazonaws.com/1.2.0/config.json";

const CONFIG_CACHE_INVALIDATION_TIMEOUT: u64 = 60 * 15;

struct DownloadedSatelliteConfig {
    config: SatelliteConfig,
    etag: String,
    downloaded_at: u64,
}

impl DownloadedSatelliteConfig {
    async fn download() -> Result<Self, Box<dyn std::error::Error>> {
        let response = reqwest::get(SATELLITE_CONFIG_FILE).await?;
        let etag = response.headers().get(reqwest::header::ETAG).unwrap().to_str().unwrap().to_string();
        let config = response.json::<SatelliteConfig>().await?;

        Ok(DownloadedSatelliteConfig {
            config,
            etag,
            downloaded_at: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
        })
    }
}

struct SatelliteConfigStore {
    current_config: Option<DownloadedSatelliteConfig>,
}

impl Default for SatelliteConfigStore {
    fn default() -> Self {
        SatelliteConfigStore {
            current_config: None,
        }
    }
}

impl SatelliteConfigStore {
    async fn update_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = DownloadedSatelliteConfig::download().await?;
        self.current_config = Some(config);
        Ok(())
    }

    async fn get_config(&mut self) -> Result<&SatelliteConfig, Box<dyn std::error::Error>> {
        let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let config = match self.current_config {
            Some(ref config) if now - config.downloaded_at < CONFIG_CACHE_INVALIDATION_TIMEOUT => {
                Ok(&config.config)
            }
            _ => {
                self.update_config().await?;
                Ok(&self.current_config.unwrap().config)
            }
        };
        config
        // if now - config.downloaded_at > CONFIG_CACHE_INVALIDATION_TIMEOUT {
        //     self.update_config().await?;
        // }

    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proj_dirs = ProjectDirs::from("com", "kydronepilot", "space-eye-rs").unwrap();
    let data_dir = proj_dirs.data_dir();
    let images_dir = data_dir.join("images");

    std::fs::create_dir_all(&images_dir).unwrap();

    let matches = Command::new("space-eye")
        .arg(
            Arg::with_name("wallpaper")
                .help("Path to the wallpaper to set")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let wallpaper_path = matches.value_of("wallpaper").unwrap();

    // println!("Hello, world!");

    // let cls = class!(NSScreen);
    // println!("NSObject size: {}", cls.instance_size());

    // let screens = NSScreen::screens();
    // <dyn NSScreen>::alloc().init();
    // unsafe {
    //     NSScreen::screens(nil);
    // }

    // set_wallpaper(1, "/Users/michael/Pictures/STSCI-J-p22031a-4000px.jpeg");
    set_wallpaper(
        1,
        wallpaper_path,
        NSColor {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        NSImageScaling::NSImageScaleProportionallyUpOrDown,
        true,
    );

    // std::process::exit(0);

    // let screens = unsafe { NSScreen::screens(nil) };
    // let count: u64 = unsafe { msg_send![screens, count] };
    // println!("{:?}", count);

    // for idx in 0..count {
    //     let screen = unsafe { screens.objectAtIndex(idx) };
    //     let description = unsafe { NSScreen::deviceDescription(screen) };
    //     let count: u64 = unsafe { msg_send![description, count] };
    //     // println!("{:?}", count);

    //     let screen_id =
    //         unsafe { NSDictionary::objectForKey_(description, *nsstring("NSScreenNumber")) };
    //     let screen_id_int: u64 = unsafe { msg_send![screen_id, unsignedIntValue] };
    //     println!("Screen id: {:?}", screen_id_int);
    //     // println!("{:?}", description);
    //     // println!("Screen {}: {:?}", idx, screen);
    //     // let screen = nsscreen_to_screen_info(screen);
    //     // virtual_rect = virtual_rect.union(&screen.rect);
    //     // by_name.insert(screen.name.clone(), screen);
    // }

    println!("{:?}", images_dir);

    let satellite_config = reqwest::get(SATELLITE_CONFIG_FILE)
        .await?
        .json::<SatelliteConfig>()
        .await?;
    println!("{:?}", satellite_config);

    let response =
        reqwest::get("https://imagery.spaceeye.app/goes-16/continental-us/5k.jpg").await?;
    std::fs::write(images_dir.join("test_sat_img.jpg"), response.bytes().await?)?;
    // let mut file = std::fs::File::create("/Users/michael/Downloads/test_sat_img.jpg")?;
    // let mut content =  Cursor::new(response.bytes().await?);
    // std::io::copy(&mut content, &mut file)?;

    let resp = reqwest::get("https://httpbin.org/ip")
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    println!("{:#?}", resp);
    Ok(())
}
