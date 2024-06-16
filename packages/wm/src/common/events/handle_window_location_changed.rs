use anyhow::Context;
use tracing::info;

use crate::{
  common::platform::NativeWindow,
  containers::{
    commands::move_container_within_tree,
    traits::{CommonGetters, PositionGetters},
  },
  user_config::{FullscreenStateConfig, UserConfig},
  windows::{
    commands::update_window_state, traits::WindowGetters, WindowState,
  },
  wm_state::WmState,
};

pub fn handle_window_location_changed(
  native_window: NativeWindow,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let found_window = state.window_from_native(&native_window);

  // Update the window's state to be fullscreen or toggled from fullscreen.
  if let Some(window) = found_window {
    let old_is_maximized = window.native().is_maximized()?;
    let is_maximized = window.native().refresh_is_maximized()?;
    let frame_position = window.native().refresh_frame_position()?;

    let nearest_monitor = state
      .nearest_monitor(&window.native())
      .context("Failed to get workspace of nearest monitor.")?;

    match window.state() {
      WindowState::Fullscreen(fullscreen_state) => {
        let is_minimized = window.native().refresh_is_minimized()?;
        let is_fullscreen =
          window.native().is_fullscreen(&nearest_monitor.to_rect()?)?;

        // A fullscreen window that gets minimized can hit this arm, so
        // ignore such events and let it be handled by the handler for
        // `PlatformEvent::WindowMinimized` instead.
        if !(is_fullscreen || is_maximized) && !is_minimized {
          info!("Window restored");

          let target_state =
            window.prev_state().unwrap_or(WindowState::Tiling);

          update_window_state(
            window.clone(),
            target_state,
            state,
            config,
          )?;
        } else if is_maximized != old_is_maximized {
          info!("Updating window's fullscreen state.");

          update_window_state(
            window.clone(),
            WindowState::Fullscreen(FullscreenStateConfig {
              maximized: is_maximized,
              ..fullscreen_state
            }),
            state,
            config,
          )?;
        }
      }
      _ => {
        // Update the window to be fullscreen if there's been a change in
        // maximized state or if the window is now fullscreen.
        if (is_maximized && old_is_maximized != is_maximized)
          || window.native().is_fullscreen(&nearest_monitor.to_rect()?)?
        {
          info!("Window fullscreened");

          update_window_state(
            window,
            WindowState::Fullscreen(FullscreenStateConfig {
              maximized: is_maximized,
              ..config.value.window_behavior.state_defaults.fullscreen
            }),
            state,
            config,
          )?;
        } else if matches!(window.state(), WindowState::Floating(_)) {
          // Update state with the new location of the floating window.
          window.set_floating_placement(frame_position);

          let workspace = window.workspace().context("No workspace.")?;

          // Get workspace that encompasses most of the window after moving.
          let updated_workspace = nearest_monitor
            .displayed_workspace()
            .context("Failed to get workspace of nearest monitor.")?;

          // Update the window's workspace if it goes out of bounds of its
          // current workspace.
          if workspace.id() != updated_workspace.id() {
            info!("Window moved to new workspace.");

            move_container_within_tree(
              window.into(),
              updated_workspace.clone().into(),
              updated_workspace.child_count(),
              state,
            )?;
          }
        }
      }
    }
  }

  Ok(())
}
