use core::ptr;

#[repr(C)]
pub struct List {
    prev: *mut List,
    next: *mut List,
}

impl List {
    pub fn init(&mut self) {
        self.prev = self;
        self.next = self;
    }

    pub unsafe fn push(&mut self, raw_addr: usize) {
        let raw_list = raw_addr as *mut List;
        ptr::write(raw_list, List {
            prev: self,
            next: self.next,
        });
        self.next.as_mut().unwrap().prev = raw_list;
        self.next = raw_list;
    }

    pub unsafe fn pop(&mut self) -> usize {
        if self.is_empty() {
            panic!("empty pop");
        }
        let raw_addr = self.next as usize;
        self.next.as_mut().unwrap().remove();
        raw_addr
    }

    pub unsafe fn remove(&mut self) {
        (*self.prev).next = self.next;
        (*self.next).prev = self.prev;
    }

    pub fn is_empty(&self) -> bool {
        ptr::eq(self.next, self)
    }
}
