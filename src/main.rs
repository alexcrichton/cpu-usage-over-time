#[macro_use]
extern crate log;

use std::thread;
use std::time::Duration;

fn main() {
    let mut cur = imp::current().unwrap();
    let dur = Duration::new(0, 100_000_000);
    loop {
        thread::sleep(dur);
        let next = imp::current().unwrap();
        let idle = imp::pct_idle(&cur, &next);
        println!("idle: {:5.02}%", idle);
        cur = next;
    }
}

#[cfg(target_os = "linux")]
mod imp {
    extern crate libc;

    use std::fs::File;
    use std::io::Read;

    pub struct State {
        user: u64,
        nice: u64,
        system: u64,
        idle: u64,
        iowait: u64,
        irq: u64,
        softirq: u64,
        steal: u64,
        guest: u64,
        guest_nice: u64,
    }

    pub fn current() -> io::Result<State> {
        let mut state = String::new();
        File::open("/proc/stat").unwrap().read_to_string(&mut state).unwrap();
        let mut parts = state.lines().next().unwrap().split_whitespace();
        assert_eq!(parts.next(), Some("cpu"));

        State {
            user: parts.next().unwrap().parse::<u64>().unwrap(),
            nice: parts.next().unwrap().parse::<u64>().unwrap(),
            system: parts.next().unwrap().parse::<u64>().unwrap(),
            idle: parts.next().unwrap().parse::<u64>().unwrap(),
            iowait: parts.next().unwrap().parse::<u64>().unwrap(),
            irq: parts.next().unwrap().parse::<u64>().unwrap(),
            softirq: parts.next().unwrap().parse::<u64>().unwrap(),
            steal: parts.next().unwrap().parse::<u64>().unwrap(),
            guest: parts.next().unwrap().parse::<u64>().unwrap(),
            guest_nice: parts.next().unwrap().parse::<u64>().unwrap(),
        }
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        let user = next.user - prev.user;
        let nice = next.nice - prev.nice;
        let system = next.system - prev.system;
        let idle = next.idle - prev.idle;
        let iowait = next.iowait - prev.iowait;
        let irq = next.irq - prev.irq;
        let softirq = next.softirq - prev.softirq;
        let steal = next.steal - prev.steal;
        let guest = next.guest - prev.guest;
        let guest_nice = next.guest_nice - prev.guest_nice;
        let total = user + nice + system + idle + iowait + irq + softirq +
            steal + guest + guest_nice;

        debug!("tick user={:5.02}% system={:5.02}% idle={:5.02}% other={:5.02}%",
               (user as f64) / (total as f64) * 100.0,
               (system as f64) / (total as f64) * 100.0,
               (idle as f64) / (total as f64) * 100.0,
               ((total - user - system - idle) as f64) / (total as f64) * 100.0);
        (idle as f64) / (total as f64) * 100.0
    }
}

#[cfg(target_os = "macos")]
#[allow(bad_style)]
mod imp {
    extern crate libc;

    use std::ptr;
    use std::slice;
    use std::time::Duration;

    type host_t = libc::c_uint;
    type mach_port_t = libc::c_uint;
    type processor_flavor_t = libc::c_int;
    type natural_t = libc::c_uint;
    type processor_info_array_t = *mut libc::c_int;
    type mach_msg_type_number_t = libc::c_int;
    type kern_return_t = libc::c_int;

    const PROESSOR_CPU_LOAD_INFO: processor_flavor_t = 2;
    const CPU_STATE_USER: usize = 0;
    const CPU_STATE_SYSTEM: usize = 1;
    const CPU_STATE_IDLE: usize = 2;
    const CPU_STATE_NICE: usize = 3;

    extern {
        fn mach_host_self() -> mach_port_t;
        fn host_processor_info(host: host_t,
                               flavor: processor_flavor_t,
                               out_processor_count: *mut natural_t,
                               out_processor_info: *mut processor_info_array_t,
                               out_processor_infoCnt: *mut mach_msg_type_number_t)
            -> kern_return_t;
    }

    pub struct State {
        user: u64,
        system: u64,
        idle: u64,
        nice: u64,
    }

    pub fn current() -> State {
        unsafe {
            let mut num_cpus_u = 0;
            let mut cpu_info = ptr::null_mut();
            let mut cpu_info_cnt = 0;
            let err = host_processor_info(
                mach_host_self(),
                PROESSOR_CPU_LOAD_INFO,
                &mut num_cpus_u,
                &mut cpu_info,
                &mut cpu_info_cnt,
            );
            if err != 0 {
                panic!("failed in host_processor_info");
            }
            let cpu_info = slice::from_raw_parts(cpu_info, cpu_info_cnt as usize);
            let mut ret = State {
                user: 0,
                system: 0,
                idle: 0,
                nice: 0,
            };
            for chunk in cpu_info.chunks(num_cpus_u as usize) {
                ret.user += chunk[CPU_STATE_USER] as u64;
                ret.system += chunk[CPU_STATE_SYSTEM] as u64;
                ret.idle += chunk[CPU_STATE_IDLE] as u64;
                ret.nice += chunk[CPU_STATE_NICE] as u64;
            }
            ret
        }
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        let user = next.user - prev.user;
        let system = next.system - prev.system;
        let idle = next.idle - prev.idle;
        let nice = next.nice - prev.nice;
        let total = user + system + idle + nice;
        debug!("tick user={:5.02}% system={:5.02}% idle={:5.02}% nice={:5.02}%",
               (user as f64) / (total as f64) * 100.0,
               (system as f64) / (total as f64) * 100.0,
               (idle as f64) / (total as f64) * 100.0,
               (nice as f64) / (total as f64) * 100.0);
        (idle as f64) / (total as f64) * 100.0
    }
}

#[cfg(windows)]
mod imp {
    extern crate winapi;

    use std::io;
    use std::mem;
    use self::winapi::um::processthreadsapi::*;
    use self::winapi::shared::minwindef::*;

    pub struct State {
        idle: FILETIME,
        kernel: FILETIME,
        user: FILETIME,
    }

    pub fn current() -> io::Result<State> {
        unsafe {
            let mut ret = mem::zeroed::<State>();
            let r = GetSystemTimes(
                &mut ret.idle,
                &mut ret.kernel,
                &mut ret.user,
            );
            if r != 0 {
                Ok(ret)
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }

    pub fn pct_idle(prev: &State, next: &State) -> f64 {
        fn to_u64(a: &FILETIME) -> u64 {
            ((a.dwHighDateTime as u64) << 32) | (a.dwLowDateTime as u64)
        }

        let idle = to_u64(&next.idle) - to_u64(&prev.idle);
        let kernel = to_u64(&next.kernel) - to_u64(&prev.kernel);
        let user = to_u64(&next.user) - to_u64(&prev.user);
        let total = user + kernel;
        println!("tick user={:5.02}% kernel={:5.02}% idle={:5.02}%",
                 (user as f64) / (total as f64) * 100.0,
                 ((kernel - idle) as f64) / (total as f64) * 100.0,
                 (idle as f64) / (total as f64) * 100.0);
        (idle as f64) / (total as f64) * 100.0
    }
}
