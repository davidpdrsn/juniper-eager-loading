use crate::{HasMany, HasManyThrough, HasOne, HasOneInner, OptionHasOne};

/// Methods available for all association types.
pub trait Association<T> {
    /// Store the loaded child on the association.
    fn loaded_child(&mut self, child: T);

    /// The association should have been loaded by now, if not store an error inside the
    /// association (if applicable for the particular association).
    fn assert_loaded_otherwise_failed(&mut self);
}

// --
// -- impl for HasOne
// --
impl<T> Association<T> for HasOne<T> {
    fn loaded_child(&mut self, child: T) {
        has_one_loaded_child(self, child)
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        has_one_assert_loaded_otherwise_failed(self)
    }
}

impl<T> Association<T> for HasOne<Box<T>> {
    fn loaded_child(&mut self, child: T) {
        has_one_loaded_child(self, Box::new(child))
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        has_one_assert_loaded_otherwise_failed(self)
    }
}

fn has_one_loaded_child<T>(association: &mut HasOne<T>, child: T) {
    std::mem::replace(&mut association.0, HasOneInner::Loaded(child));
}

fn has_one_assert_loaded_otherwise_failed<T>(association: &mut HasOne<T>) {
    association.0.assert_loaded_otherwise_failed()
}

// --
// -- impl for OptionHasOne
// --
impl<T> Association<T> for OptionHasOne<T> {
    fn loaded_child(&mut self, child: T) {
        option_has_one_loaded_child(self, Some(child));
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        option_has_one_assert_loaded_otherwise_failed(self)
    }
}

impl<T> Association<T> for OptionHasOne<Box<T>> {
    fn loaded_child(&mut self, child: T) {
        option_has_one_loaded_child(self, Some(Box::new(child)));
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        option_has_one_assert_loaded_otherwise_failed(self)
    }
}

fn option_has_one_loaded_child<T>(association: &mut OptionHasOne<T>, child: Option<T>) {
    std::mem::replace(&mut association.0, child);
}

fn option_has_one_assert_loaded_otherwise_failed<T>(association: &mut OptionHasOne<T>) {
    match association.0 {
        Some(_) => {}
        None => {
            std::mem::replace(&mut association.0, None);
        }
    }
}

// --
// -- impl for HasMany
// --
impl<T> Association<T> for HasMany<T> {
    fn loaded_child(&mut self, child: T) {
        self.0.push(child);
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        // cannot fail, defaults to an empty vec
    }
}

// --
// -- impl for HasManyThrough
// --
impl<T> Association<T> for HasManyThrough<T> {
    fn loaded_child(&mut self, child: T) {
        self.0.push(child);
    }

    fn assert_loaded_otherwise_failed(&mut self) {
        // cannot fail, defaults to an empty vec
    }
}

// NOTE: We don't have to implement Association for HasMany<Box<T>> or HasManyThrough<Box<T>>
// because they already have indirection through the inner Vec. So recursive types are supported.
