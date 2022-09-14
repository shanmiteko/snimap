#[macro_export]
macro_rules! by {
    ($byte:expr) => {
        |b| b == $byte
    };
}

#[macro_export]
macro_rules! const_enum {
    {
        $vis:vis enum $BytesEnum:ident: $T:ty {
            $(
                $Flag:ident = $value:expr,
            )*
        }
    } => {
        $vis struct $BytesEnum {
            $vis inner: $T
        }

        #[allow(non_upper_case_globals)]
        impl $BytesEnum {
            $($vis const $Flag: Self = Self { inner: $value };)*
        }
    };
}
