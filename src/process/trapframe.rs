//! trap frame is used to store register states, when doing user-kernel space switching 

#[repr(C)]
pub struct TrapFrame {
    /*   0 */ pub kernel_satp: usize,   // kernel page table
    /*   8 */ pub kernel_sp: usize,     // top of process's kernel stack
    /*  16 */ pub kernel_trap: usize,   // usertrap()
    /*  24 */ pub epc: usize,           // saved user program counter
    /*  32 */ pub kernel_hartid: usize, // saved kernel tp
    /*  40 */ ra: usize,
    /*  48 */ sp: usize,
    /*  56 */ gp: usize,
    /*  64 */ tp: usize,
    /*  72 */ t0: usize,
    /*  80 */ t1: usize,
    /*  88 */ t2: usize,
    /*  96 */ s0: usize,
    /* 104 */ s1: usize,
    /* 112 */ a0: usize,
    /* 120 */ a1: usize,
    /* 128 */ a2: usize,
    /* 136 */ a3: usize,
    /* 144 */ a4: usize,
    /* 152 */ a5: usize,
    /* 160 */ a6: usize,
    /* 168 */ a7: usize,
    /* 176 */ s2: usize,
    /* 184 */ s3: usize,
    /* 192 */ s4: usize,
    /* 200 */ s5: usize,
    /* 208 */ s6: usize,
    /* 216 */ s7: usize,
    /* 224 */ s8: usize,
    /* 232 */ s9: usize,
    /* 240 */ s10: usize,
    /* 248 */ s11: usize,
    /* 256 */ t3: usize,
    /* 264 */ t4: usize,
    /* 272 */ t5: usize,
    /* 280 */ t6: usize,
}

impl TrapFrame {
    #[inline]
    pub fn admit_ecall(&mut self) {
        self.epc += 4;
    }

    #[inline]
    pub fn set_sp(&mut self, sp: usize) {
        self.sp = sp;
    }

    #[inline]
    pub fn set_a0(&mut self, a0: usize){
        self.a0 = a0;
    }

    #[inline]
    pub fn get_a0(&self) -> usize {
        self.a0
    }

    #[inline]
    pub fn get_a1(&self) -> usize {
        self.a1
    }

    #[inline]
    pub fn get_a2(&self) -> usize {
        self.a2
    }

    #[inline]
    pub fn get_a3(&self) -> usize {
        self.a3
    }

    #[inline]
    pub fn get_a4(&self) -> usize {
        self.a4
    }

    #[inline]
    pub fn get_a5(&self) -> usize {
        self.a5
    }

    #[inline]
    pub fn get_a7(&self) -> usize {
        self.a7
    }
}
