pub mod macros {
    macro_rules! declare_node {
        (
            name: $node_name:ident,
            fields: {
                #[entity] $entity_field:ident: Entity,
                $(#[input] $input_field:ident: $input_type:ty,)*
                $(#[output] $output_field:ident: $output_type:ty,)*
                $($regular_field:ident: $regular_type:ty,)*
            },
            methods: {
                new(
                    $($param_name:ident: $param_type:ty),* $(,)?
                ) -> Self $constructor_body:block
                process($($process_args:tt)*) $process_body:block
                $(set_input($($set_input_args:tt)*) -> Result<(), String> $set_input_body:block)?
            }
        ) => {
            #[derive(Clone)]
            pub struct $node_name {
                pub $entity_field: Entity,
                $(pub $input_field: $input_type,)*
                $(pub $output_field: $output_type,)*
                $(pub $regular_field: $regular_type,)*
            }

            impl $node_name {
                pub fn new($($param_name: $param_type),*) -> Self $constructor_body

                $(pub const $input_field: $crate::nodes::InputId = $crate::nodes::InputId(stringify!($node_name), stringify!($input_field));)*
                $(pub const $output_field: $crate::nodes::OutputId = $crate::nodes::OutputId(stringify!($node_name), stringify!($output_field));)*

                $(fn custom_set_input($($set_input_args)*) -> Result<(), String> $set_input_body)?

                fn convert_input(&self, id: $crate::nodes::InputId, value: $crate::nodes::Field) -> Result<$crate::nodes::Field, String> {
                    match id {
                        $(Self::$input_field => Ok($crate::nodes::Field::from(<$input_type>::try_from(value)?)),)*
                        _ => Err(format!("Invalid input field ID for {}", stringify!($node_name))),
                    }
                }
            }

            impl $crate::nodes::NodeTrait for $node_name {
                fn get_input(&self, id: $crate::nodes::InputId) -> Option<$crate::nodes::Field> {
                    match id {
                        $(Self::$input_field => Some($crate::nodes::Field::from(self.$input_field.clone())),)*
                        _ => None,
                    }
                }

                fn get_output(&self, id: $crate::nodes::OutputId) -> Option<$crate::nodes::Field> {
                    match id {
                        $(Self::$output_field => Some($crate::nodes::Field::from(self.$output_field.clone())),)*
                        _ => None,
                    }
                }

                fn set_input(&mut self, id: $crate::nodes::InputId, value: $crate::nodes::Field) -> Result<(), String> {
                    let converted_value = self.convert_input(id, value)?;
                    declare_node!(@optional_set_input, self, id, &converted_value, $($($set_input_args)*)?);
                    match id {
                        $(Self::$input_field => {
                            self.$input_field = <$input_type>::try_from(converted_value)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid input field ID for {}", stringify!($node_name))),
                    }
                }

                fn set_output(&mut self, id: $crate::nodes::OutputId, value: $crate::nodes::Field) -> Result<(), String> {
                    match id {
                        $(Self::$output_field => {
                            self.$output_field = <$output_type>::try_from(value)?;
                            Ok(())
                        })*
                        _ => Err(format!("Invalid output field ID for {}", stringify!($node_name))),
                    }
                }

                fn input_fields(&self) -> &[$crate::nodes::InputId] {
                    &[$(Self::$input_field,)*]
                }

                fn output_fields(&self) -> &[$crate::nodes::OutputId] {
                    &[$(Self::$output_field,)*]
                }

                async fn process($($process_args)*) $process_body

                fn entity(&self) -> Entity {
                    self.$entity_field
                }
            }
        };

        (@optional_set_input, $self:expr, $id:expr, $converted_value:expr, $($args:tt)+) => {
            $self.custom_set_input($id, $converted_value)?;
        };

        (@optional_set_input, $self:expr, $id:expr, $converted_value:expr,) => {
            // Do nothing if there's no custom set_input implementation
        };
    }

    macro_rules! define_field_enum {
        (
            $(#[$meta:meta])*
            $vis:vis enum $name:ident {
                $(
                    $variant:ident($type:ty)
                ),* $(,)?
            }
        ) => {
            $(#[$meta])*
            $vis enum $name {
                $($variant($type),)*
            }

            $(
                impl From<$type> for $name {
                    fn from(value: $type) -> Self {
                        $name::$variant(value)
                    }
                }
            )*
        }
    }

    macro_rules! declare_node_enum_and_impl_trait {
        (
            $(#[$meta:meta])*
            $vis:vis enum $enum_name:ident {
                $($variant:ident($node_type:ty)),* $(,)?
            }
        ) => {
            #[derive(Clone)]
            $(#[$meta])*
            $vis enum $enum_name {
                $($variant($node_type)),*
            }

            #[derive(Event, Debug, Clone)]
            pub enum RequestSpawnNodeKind {
                $($variant,)*
            }

            #[derive(Event, Debug, Clone)]
            pub struct RequestSpawnNode {
                pub position: Vec2,
                pub kind: RequestSpawnNodeKind,
            }

            impl NodeTrait for $enum_name {
                fn get_input(&self, id: InputId) -> Option<Field> {
                    match self {
                        $($enum_name::$variant(n) => n.get_input(id),)*
                    }
                }

                fn get_output(&self, id: OutputId) -> Option<Field> {
                    match self {
                        $($enum_name::$variant(n) => n.get_output(id),)*
                    }
                }

                fn set_input(&mut self, id: InputId, value: Field) -> Result<(), String> {
                    match self {
                        $($enum_name::$variant(n) => n.set_input(id, value),)*
                    }
                }

                fn set_output(&mut self, id: OutputId, value: Field) -> Result<(), String> {
                    match self {
                        $($enum_name::$variant(n) => n.set_output(id, value),)*
                    }
                }

                fn input_fields(&self) -> &[InputId] {
                    match self {
                        $($enum_name::$variant(n) => n.input_fields(),)*
                    }
                }

                fn output_fields(&self) -> &[OutputId] {
                    match self {
                        $($enum_name::$variant(n) => n.output_fields(),)*
                    }
                }

                async fn process(&mut self, render_device: &CustomGpuDevice, render_queue: &CustomGpuQueue) {
                    match self {
                        $($enum_name::$variant(n) => n.process(render_device, render_queue).await,)*
                    }
                }

                fn entity(&self) -> Entity {
                    match self {
                        $($enum_name::$variant(n) => n.entity(),)*
                    }
                }
            }
        }
    }

    pub(crate) use declare_node;
    pub(crate) use declare_node_enum_and_impl_trait;
    pub(crate) use define_field_enum;
}
