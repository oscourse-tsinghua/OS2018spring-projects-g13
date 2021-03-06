use std::collections::VecDeque;
use allocator::imp::BACKUP_ALLOCATOR;
use mutex::Mutex;
use process::{Process, State, Id};
use traps::TrapFrame;
use ALLOCATOR;
use pi::interrupt::{Interrupt, Controller};
use pi::timer::tick_in;
use std::ops::Deref;
use aarch64;
use shell;
use std::mem;

use console::kprintln;

use allocator::imp::USER_ALLOCATOR;

// use console;

/// The `tick` time.
// FIXME: When you're ready, change this to something more reasonable.
pub const TICK: u32 = 10 * 1000 * 10;

/// Process scheduler for the entire machine.
// #[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.0.lock().as_mut().expect("scheduler uninitialized").add(process)
    }

    pub fn clear(&self) {
        *self.0.lock() = None;
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::switch()`.
    #[must_use]
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Option<Id> {
        self.0.lock().as_mut().expect("scheduler uninitialized").switch(new_state, tf)
    }

    // pub fn is_finished(&self, pending_pid: usize) -> bool {
    //     self.0.lock().as_mut().expect("scheduler uninitialized").is_finished(pending_pid)
    // }

    pub fn is_empty(&self) -> bool {
        self.0.lock().as_mut().expect("scheduler uninitialized").is_empty()
    }

    pub fn pop_current(&self) -> Process {
        self.0.lock().as_mut().expect("scheduler uninitialized").pop_current()
    }

    pub fn push_current_front(&self, process: Process) {
        self.0.lock().as_mut().expect("scheduler uninitialized").push_current_front(process)
    }
    
    pub fn last_id(&self) -> u64 {
        self.0.lock().as_mut().expect("scheduler uninitialized").last_id()
    }

    /// Initializes the scheduler and starts executing processes in user space
    /// using timer interrupt based preemptive scheduling. This method should
    /// not return under normal conditions.
    pub fn start(&self, addr: usize) {
        *self.0.lock() = Some(Scheduler::new());
        // let mut process = Process::new().unwrap();
        // process.trap_frame.ttbr0 = 0x01000000;
        // process.trap_frame.sp = process.stack.top().as_u64();
        // process.trap_frame.elr = shell_thread as *mut u8 as u64;
        // process.trap_frame.spsr = 0b000; // To EL 0, currently only unmasking IRQ
        // let tf = process.trap_frame.clone();
        // self.add(process);

        // let mut process2 = Process::new().unwrap();
        // process2.trap_frame.sp = process2.stack.top().as_u64();
        // process2.trap_frame.elr = shell_thread_2 as *mut u8 as u64;
        // // process2.trap_frame.spsr = 0b1101_00_0000; // To EL 0, currently only unmasking IRQ
        // self.add(process2);

        // // Controller::new().enable(Interrupt::Timer1);
        // // tick_in(TICK);

        // // let tf_addr = Box::into_raw(tf) as *mut usize as usize;
        // // kprintln!("trapframe: {:x}", tf_addr);
        // unsafe { kprintln!("ttbr0 ttbr1: {:x} {:x}", get_ttbr0(), get_ttbr1()); }
        // unsafe { kprintln!("pc: {:x}", get_pc()); }
        // kprintln!("=========== ready to switch to user process: {:x} ===========", PADDR(shell_thread as *mut u8 as usize) as u64);
        
        // // shell_thread();
        // kprintln!("instruction: {:x}", unsafe { *(shell_thread as *mut u32) });

        // unsafe {
        //     asm!("mov sp, $0
        //       bl context_restore
        //       adr lr, _start
        //       mov sp, lr
        //       mov lr, xzr
        //       eret" :: "r"(tf) :: "volatile");
        // };

        // kprintln!("start schedule");
        
        let mut process = Process::new();
        // process.trap_frame.ttbr0 = 0x01000000;
        // process.trap_frame.sp = process.stack.top().as_u64();
        process.trap_frame.elr = (0x4) as *mut u8 as u64;
        process.trap_frame.spsr = 0b000; // To EL 0, currently only unmasking IRQ
        process.load_icode(addr  as *mut u8);
        let tf = process.trap_frame.clone();
        let allocator = Box::new(process.allocator);
        self.add(process);

        // kprintln!("add process");

        // let mut process2 = Process::new();
        // process2.trap_frame.ttbr0 = 0x01000000;
        // // process.trap_frame.sp = process.stack.top().as_u64();
        // process2.trap_frame.elr = (0x4) as *mut u8 as u64;
        // process2.trap_frame.spsr = 0b000; // To EL 0, currently only unmasking IRQ
        // process2.load_icode((0x1510000)  as *mut u8);
        // self.add(process2);
        
        Controller::new().enable(Interrupt::Timer1);
        tick_in(TICK);

        ALLOCATOR.switch_content(allocator.deref(), unsafe { &mut BACKUP_ALLOCATOR });

        // kprintln!("========================================enter user mode========================================");
        unsafe {
            asm!("mov sp, $0
              bl context_restore
              adr lr, _start
              mov sp, lr
              mov lr, xzr
              dsb ishst
              tlbi vmalle1is
              dsb ish
              tlbi vmalle1is
              isb
              eret" :: "r"(tf) :: "volatile");
        };

        // sys_exec(1);
        // kprintln!("no eret");
    }
}

#[derive(Debug)]
struct Scheduler{
    processes: VecDeque<Process>,
    current: Option<Id>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::new(),
            current: None,
            last_id: None,
        }
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// If this is the first process added, it is marked as the current process.
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        let id = match self.last_id {
            Some(last_id) => last_id.checked_add(1)?,
            None => 0
        };

        process.trap_frame.tpidr = id;
        self.processes.push_back(process);

        if let None = self.current {
            self.current = Some(id);
        }

        self.last_id = Some(id);
        self.last_id
    }

    fn last_id(&self) -> u64 {
        self.last_id.expect("no process to fork")
    }

    /// Sets the current process's state to `new_state`, finds the next process
    /// to switch to, and performs the context switch on `tf` by saving `tf`
    /// into the current process and restoring the next process's trap frame
    /// into `tf`. If there is no current process, returns `None`. Otherwise,
    /// returns `Some` of the process ID that was context switched into `tf`.
    ///
    /// This method blocks until there is a process to switch to, conserving
    /// energy as much as possible in the interim.
    fn switch(&mut self, new_state: State, tf: &mut TrapFrame) -> Option<Id> {
        let mut current = self.processes.pop_front()?;
        let current_id = current.get_id();
        current.trap_frame = Box::new(*tf);
        current.state = new_state;
        self.processes.push_back(current);

        loop {
            let mut zombie_all = true;
            let mut process = self.processes.pop_front()?;
            
            let state = mem::replace(&mut process.state, State::Ready);
            if let State::Wait_Proc(mut id) = state {
                // kprintln!("try wait");
                process.state = State::Wait_Proc(id);
                let wait_father_id = process.get_id();
                self.processes.push_back(process);

                let mut wait_finish = false;

                loop {
                    let mut sub_process = self.processes.pop_front()?;
                    if let State::Zombie = sub_process.state {
                        if sub_process.get_id() == id as u64 {
                            wait_finish = true;
                        }
                    }

                    if sub_process.get_id() == wait_father_id {
                        self.processes.push_front(sub_process);
                        break;
                    }
                    self.processes.push_back(sub_process);
                }

                let mut sub_process = self.processes.pop_front()?;
                if wait_finish {
                    kprintln!("finish wait"); 
                    self.current = Some(sub_process.get_id() as Id);
                    *tf = *sub_process.trap_frame;
                    unsafe { USER_ALLOCATOR = sub_process.allocator; }
                    sub_process.state = State::Running;

                    // Push process back into queue.
                    self.processes.push_front(sub_process);
                    break;
                }
                else {
                    self.processes.push_back(sub_process);
                    continue;
                }
            }

            process.state = state;

            if process.is_ready() {
                self.current = Some(process.get_id() as Id);
                *tf = *process.trap_frame;
                unsafe { USER_ALLOCATOR = process.allocator; }
                process.state = State::Running;

                // Push process back into queue.
                self.processes.push_front(process);
                break;
            } else if process.not_zombie() {
                zombie_all = false;
            } else if process.get_id() == current_id && zombie_all {
                // We cycled the list, wait for an interrupt.
                return None;
                // aarch64::wfi();
            }

            self.processes.push_back(process);
        }
        // kprintln!("current: {}", self.current.unwrap()); 
        self.current
    }

    // fn is_finished(&self, pending_pid: usize) -> bool {
    //     for process in self.processes.iter() {
    //         if (*process).pid == pending_pid {
    //             return false;
    //         }
    //     }
    //     true
    // }
    
    fn is_empty(&self) -> bool {
        self.processes.is_empty()
    }

    fn pop_current(&mut self) -> Process {
        self.processes.pop_front().expect("no processes running.")
    }

    fn push_current_front(&mut self, process: Process) {
        self.processes.push_front(process);
    }
}
