use std::time::Duration;
static mut START: Option<std::time::Instant> = None;

fn start_time() -> std::time::Instant {
    match unsafe { START } {
        Some(start) => start,
        None => {
            let now = std::time::Instant::now();
            unsafe { START = Some(now) }
            now
        }
    }
}

pub fn now() -> fugit::Instant<u32, 1, 1000> {
    let now = std::time::Instant::now();
    let start = start_time();
    let dur = now.duration_since(start);
    fugit::Instant::<u32, 1, 1000>::from_ticks(dur.as_millis() as u32)
}

pub fn delay(d: fugit::MillisDurationU32) {
    let dur = Duration::from_millis(d.to_millis() as u64);
    std::thread::sleep(dur);
}
