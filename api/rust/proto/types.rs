//! New-types which can be converted from/into proto types (e.g. i32).

/// Defines a new-type for id.
macro_rules! id_new_type {
    ($new_type:ident($inner_type:ty)) => {
        #[derive(Debug, Clone, Copy, derive_more::Into, derive_more::From, PartialEq, Eq, Hash)]
        pub struct $new_type($inner_type);

        impl $new_type {
            pub fn new(value: $inner_type) -> Self {
                Self(value)
            }

            /// Returns inner value of self.
            pub fn into_inner(self) -> $inner_type {
                <$new_type as Into<$inner_type>>::into(self)
            }
        }
    };
}

id_new_type!(PlayerId(u32));

id_new_type!(GameId(u32));

id_new_type!(MachineId(u32));
