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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let resp = reqwest::get("https://httpbin.org/ip")
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    println!("{:#?}", resp);
    Ok(())
}
