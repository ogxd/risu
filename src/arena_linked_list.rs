pub struct ArenaLinkedList<T> {
    count: usize,
    first_index: usize,
    last_index: usize,
    first_free_node_index: usize,
    array: Vec<ArenaLinkedListNode<T>>,
}

impl<T> ArenaLinkedList<T> where T: Default {
  
    pub fn new_with_capacity(capacity: usize) -> Self {
        let mut array = Vec::with_capacity(capacity);
        array.resize_with(capacity, || ArenaLinkedListNode::new(T::default()));
        let mut new = Self {
            count: 0,
            first_index: usize::MAX,
            last_index: usize::MAX,
            first_free_node_index: 0,
            array: array,
        };
        new.clear();
        new
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn clear(&mut self)
    {
        self.fill_free(0, self.array.len());
        self.count = 0;
        self.first_index = usize::MAX;
        self.last_index = usize::MAX;
    }

    fn fill_free(&mut self, start: usize, count: usize)
    {
        self.first_free_node_index = start;
        for i in start..(start + count - 1)
        {
            self.array[i].after = i + 1;
        }
        self.array[start + count - 1].after = usize::MAX;
    }

    pub fn get(&self, index: usize) -> Result<&ArenaLinkedListNode<T>, ()>
    {
        let node = &self.array[index];
        if !node.used {
            return Err(());
        }
        Ok(node)
    }

    pub fn remove(&mut self, index: usize) -> Result<(), ()>
    {
        if index == usize::MAX || index > self.array.len() {
            return Err(());
        }
        let node = &mut self.array[self.first_index];

        if !node.used {
            return Err(());
        }

        let node_after = node.after;
        let node_before = node.before;

        // Mark node as free
        node.used = false;
        node.value = T::default(); // Free reference
        node.before = usize::MAX;
        node.after = self.first_free_node_index;
        self.first_free_node_index = index;

        // Remap links
        if node_before == usize::MAX {
            self.first_index = node_after;
        } else {
            let before_node = &mut self.array[node_before]; // cannot borrow `self.array` as mutable more than once at a time
            before_node.after = node_after;
        }

        if node_after == usize::MAX {
            self.last_index = node_before;
        } else {
            let after_node = &mut self.array[node_after]; // cannot borrow `self.array` as mutable more than once at a time
            after_node.before = node_before;
        }

        // Decrement count
        self.count -= 1;

        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct ArenaLinkedListNode<T> {
    used: bool,
    before: usize,
    after: usize,
    value: T
}

impl<T> ArenaLinkedListNode<T> {
    pub fn new(value: T) -> Self {
        Self {
            used: true,
            before: usize::MAX,
            after: usize::MAX,
            value: value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_linked_list() {
        let list = ArenaLinkedList::<u32>::new_with_capacity(10);
        assert_eq!(list.count(), 0);
    }
}