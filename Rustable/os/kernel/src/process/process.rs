use traps::TrapFrame;
use process::{State, Stack};

// use console;
use std::mem;

/// Type alias for the type of a process ID.
pub type Id = u64;

/// A structure that represents the complete state of a process.
#[derive(Debug)]
pub struct Process {
    /// The saved trap frame of a process.
    pub trap_frame: Box<TrapFrame>,
    /// The memory allocation used for the process's stack.
    pub stack: Stack,
    /// The scheduling state of the process.
    pub state: State,
}

impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `None`. Otherwise returns `Some` of the new `Process`.
    pub fn new() -> Option<Process> {
        match Stack::new() {
            Some(stack) => Some(Process {
                trap_frame: Box::new(TrapFrame::default()),
                stack,
                state: State::Ready,
            }),
            None => None,
        }
    }

    pub fn get_id(&self) -> u64 {
        self.trap_frame.tpidr
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occured. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        if let State::Ready = self.state {
            true
        } else if let State::Running = self.state {
            false
        } else {
            let state = mem::replace(&mut self.state, State::Ready);
            if let State::Waiting(mut event_poll_fn) = state {
                if event_poll_fn(self) {
                    true
                } else {
                    self.state = State::Waiting(event_poll_fn);
                    false
                }
            } else {
                unreachable!();
            }
        }
    }
}



fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.offset(i as isize) = *src.offset(i as isize);
        i += 1;
    }
    return dest;
}

fn map(va: usize, mem_size: u32, file_size: u32, phoff: usize) {
    let offset = va - align_down(va, PGSIZE);
    if offset > 0 {
        let page = match alloc_pages(1) {
            Ok(page) => { page },
            Err(_) => {return Err(-1); }
        }
        let src = phoff as *mut u8;
        let dst = page2kva(page) + offset
        memcpy(src, dest, PGISZE - offset);
        
    }
}


static int load_icode_mapper(u_long va, u_int32_t sgsize,
                             u_char *bin, u_int32_t bin_size, void *user_data)
{
    struct Env *env = (struct Env *)user_data;
    struct Page *p = NULL;
    u_long i;
    int r;
    u_long offset = va - ROUNDDOWN(va, BY2PG);
    
    /*Step 1: load all content of bin into memory. */
    if (offset)
    {
        /* Hint: You should alloc a page and increase the reference count of it. */
        if ((r = page_alloc(&p)) < 0)
        {
            return r;
        }
        
        char *src = (char *)((u_long)bin);
        char *dest = (char *)(page2kva(p) + offset);
        bcopy(src, dest, BY2PG - offset);
        char *temp_va = (char *)(va - offset);
        page_insert(env->env_ttbr0, p, (u_long)temp_va, ATTRIB_AP_RW_ALL);
    }
    
    for (i = offset; i < bin_size; i += BY2PG)
    {
        /* Hint: You should alloc a page and increase the reference count of it. */
        if ((r = page_alloc(&p)) < 0)
        {
            return r;
        }
        if (bin_size - i >= BY2PG)
        {
            bcopy(bin + i, (void *)page2kva(p), BY2PG);
        }
        else
        {
            bcopy(bin + i, (void *)page2kva(p), bin_size - i);
        }
        
        page_insert(env->env_ttbr0, p, va + i, ATTRIB_AP_RW_ALL);
    }
    
    /*Step 2: alloc pages to reach `sgsize` when `bin_size` < `sgsize`.
     * i has the value of `bin_size` now. */
    while (i < sgsize)
    {
        if ((r = page_alloc(&p)) < 0)
        {
            return r;
        }
        
        page_insert(env->env_ttbr0, p, va + i, ATTRIB_AP_RW_ALL);
        
        i += BY2PG;
    }
    
    return 0;
}