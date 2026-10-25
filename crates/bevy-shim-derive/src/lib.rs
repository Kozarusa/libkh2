//! `#[derive(Resource)]` that expands to nothing. The kh2-rs modules libkh2 compiles in place
//! (`motor.rs`, `world.rs`) derive Bevy's `Resource` on plain structs; outside Bevy the marker
//! has no meaning, so the derive emits no code.

use proc_macro::TokenStream;

#[proc_macro_derive(Resource)]
pub fn derive_resource(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// `#[derive(Component)]`: nothing (plain structs outside an ECS).
#[proc_macro_derive(Component)]
pub fn derive_component(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// `#[derive(Message)]`: nothing.
#[proc_macro_derive(Message)]
pub fn derive_message(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
