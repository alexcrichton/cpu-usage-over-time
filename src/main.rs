extern crate num_cpus;

use std::thread;
use std::time::Duration;

fn main() {
    let mut cur = imp::current();
    let dur = Duration::new(0, 100_000_000);
    let cpus = num_cpus::get();
    loop {
        thread::sleep(dur);
        let next = imp::current();
        imp::print(cpus, &dur, &cur, &next);
        cur = next;
    }
}

#[cfg(target_os = "linux")]
mod imp {
    extern crate libc;

    use std::fs::File;
    use std::io::Read;
    use std::time::Duration;

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

    pub fn current() -> State {
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

    pub fn print(cpus: usize, dur: &Duration, prev: &State, next: &State) {
        let clk_hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        let dur_nanos = dur.as_secs() * 1_000_000_000 +
            (dur.subsec_nanos() as u64);
        let tick_nanos = 1_000_000_000 / (clk_hz as u64);
        let ticks = dur_nanos / tick_nanos;
        println!("yay: {} {}", ticks, cpus);
        println!("\t user {}", next.user - prev.user);
        println!("\t nice {}", next.nice - prev.nice);
        println!("\t system {}", next.system - prev.system);
        println!("\t idle {}", next.idle - prev.idle);
        println!("\t iowait {}", next.iowait - prev.iowait);
        println!("\t irq {}", next.irq - prev.irq);
        println!("\t softirq {}", next.softirq - prev.softirq);
        println!("\t steal {}", next.steal - prev.steal);
        println!("\t guest {}", next.guest - prev.guest);
        println!("\t guest_nice {}", next.guest_nice - prev.guest_nice);
    }
}
