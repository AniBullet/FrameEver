use super::*;
use crate::i18n::tc;
use frameever_project::ClipTransitionType;
use frameever_rendering::{TransitionComposite, TransitionDip, TransitionLayer};

pub(super) const TRANSITION_POPOVER_WIDTH: f32 = 324.;
const TRANSITION_POPOVER_HEIGHT_ESTIMATE: f32 = 330.;
const TRANSITION_EDGE_PX: f32 = 6.;
const TRANSITION_ADD_SIZE: f32 = 18.;
const TRANSITION_DIAMOND_SIZE: f32 = 12.;
const THUMBNAIL_WIDTH: f32 = 58.;
const THUMBNAIL_HEIGHT: f32 = 36.;
const THUMBNAIL_INSET: f32 = 5.;
const PREVIEW_LOOP: Duration = Duration::from_millis(1600);
pub(super) const DEFAULT_TRANSITION_SECONDS: f64 = 0.5;

#[derive(Clone, Copy, Debug)]
pub(super) struct TransitionPopover {
    pub(super) index: usize,
    left: Pixels,
    anchor: ClipMenuAnchor,
    hovered: Option<ClipTransitionType>,
    clips: [TransitionClipKey; 2],
}

type TransitionClipKey = (u32, u64, u64);

fn transition_clip_keys(
    timeline: &TimelineConfiguration,
    index: usize,
) -> Option<[TransitionClipKey; 2]> {
    let key = |segment: &frameever_project::TimelineSegment| {
        (
            segment.recording_clip,
            segment.start.to_bits(),
            segment.end.to_bits(),
        )
    };
    Some([
        key(timeline.segments.get(index.checked_sub(1)?)?),
        key(timeline.segments.get(index)?),
    ])
}

pub(crate) fn transition_label(kind: ClipTransitionType) -> &'static str {
    match kind {
        ClipTransitionType::CrossFade => t!("Cross dissolve"),
        ClipTransitionType::FadeThroughBlack => t!("Dip to black"),
        ClipTransitionType::FadeThroughWhite => t!("Dip to white"),
        ClipTransitionType::PushLeft => t!("Push left"),
        ClipTransitionType::PushRight => t!("Push right"),
        ClipTransitionType::PushUp => t!("Push up"),
        ClipTransitionType::PushDown => t!("Push down"),
        ClipTransitionType::WipeLeft => t!("Wipe left"),
        ClipTransitionType::WipeRight => t!("Wipe right"),
        ClipTransitionType::WipeUp => t!("Wipe up"),
        ClipTransitionType::WipeDown => t!("Wipe down"),
        ClipTransitionType::Zoom => t!("Zoom"),
    }
}

pub(super) fn transition_title() -> &'static str {
    tc!("clip transition", "Transition")
}

fn preview_progress(loop_position: f32) -> f32 {
    ((loop_position - 0.2) / 0.6).clamp(0., 1.)
}

fn transition_thumbnail(theme: &Theme, kind: ClipTransitionType, progress: f32) -> gpui::Div {
    let outgoing = Hsla::from(theme.border_strong);
    let incoming = Hsla::from(theme.muted_foreground);
    let frame = div()
        .relative()
        .flex_none()
        .w(px(THUMBNAIL_WIDTH))
        .h(px(THUMBNAIL_HEIGHT))
        .rounded(px(4.))
        .overflow_hidden()
        .bg(Hsla::from(theme.muted));
    let fill = |color: Hsla, opacity: f32| {
        div()
            .absolute()
            .inset_0()
            .bg(color)
            .opacity(opacity.clamp(0., 1.))
    };
    let card = |color: Hsla, layer: TransitionLayer, origin: (f32, f32)| {
        let width = (THUMBNAIL_WIDTH - THUMBNAIL_INSET * 2.) * layer.scale;
        let height = (THUMBNAIL_HEIGHT - THUMBNAIL_INSET * 2.) * layer.scale;
        div()
            .absolute()
            .left(px((THUMBNAIL_WIDTH - width) / 2.
                + layer.offset[0] * THUMBNAIL_WIDTH
                - origin.0))
            .top(px((THUMBNAIL_HEIGHT - height) / 2.
                + layer.offset[1] * THUMBNAIL_HEIGHT
                - origin.1))
            .w(px(width))
            .h(px(height))
            .rounded(px(2.))
            .bg(color)
            .opacity(layer.opacity.clamp(0., 1.))
    };
    match TransitionComposite::at(kind, progress) {
        TransitionComposite::Frames {
            outgoing_weight,
            incoming_weight,
            dip,
        } => match dip {
            None => frame
                .child(fill(outgoing, 1.))
                .child(fill(incoming, incoming_weight)),
            Some(dip) => frame
                .child(fill(
                    match dip {
                        TransitionDip::Black => gpui::black(),
                        TransitionDip::White => gpui::white(),
                    },
                    1.,
                ))
                .child(fill(outgoing, outgoing_weight))
                .child(fill(incoming, incoming_weight)),
        },
        TransitionComposite::Layers {
            outgoing: outgoing_layer,
            incoming: incoming_layer,
            wipe,
        } => {
            let frame = frame.child(card(outgoing, outgoing_layer, (0., 0.)));
            let Some(wipe) = wipe else {
                return frame.child(card(incoming, incoming_layer, (0., 0.)));
            };
            let edge = wipe.edge.clamp(0., 1.);
            let (left, top, width, height) = match wipe.direction {
                [x, _] if x > 0. => (0., 0., edge * THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT),
                [x, _] if x < 0. => (
                    (1. - edge) * THUMBNAIL_WIDTH,
                    0.,
                    edge * THUMBNAIL_WIDTH,
                    THUMBNAIL_HEIGHT,
                ),
                [_, y] if y > 0. => (0., 0., THUMBNAIL_WIDTH, edge * THUMBNAIL_HEIGHT),
                _ => (
                    0.,
                    (1. - edge) * THUMBNAIL_HEIGHT,
                    THUMBNAIL_WIDTH,
                    edge * THUMBNAIL_HEIGHT,
                ),
            };
            frame.child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .h(px(height))
                    .overflow_hidden()
                    .child(card(incoming, incoming_layer, (left, top))),
            )
        }
    }
}

fn diamond(color: Hsla, size: f32) -> impl IntoElement {
    gpui::canvas(
        |bounds, _window, _cx| bounds,
        move |_, bounds, window, _cx| {
            let half = size / 2.;
            let mut builder = gpui::PathBuilder::fill();
            let point =
                |x: f32, y: f32| gpui::point(bounds.origin.x + px(x), bounds.origin.y + px(y));
            builder.move_to(point(half, 0.));
            builder.line_to(point(size, half));
            builder.line_to(point(half, size));
            builder.line_to(point(0., half));
            builder.close();
            if let Ok(path) = builder.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size(px(size))
}

fn popover_placement(
    origin: Point<Pixels>,
    viewport: gpui::Size<Pixels>,
) -> (Pixels, ClipMenuAnchor) {
    let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
    let left = px((f32::from(origin.x) - TRANSITION_POPOVER_WIDTH / 2.)
        .min(width - TRANSITION_POPOVER_WIDTH - 12.)
        .max(12.));
    let y = f32::from(origin.y);
    let room_above = y - 20.;
    let room_below = height - y - 20.;
    let anchor = if room_above >= TRANSITION_POPOVER_HEIGHT_ESTIMATE || room_above >= room_below {
        ClipMenuAnchor::Above {
            bottom: px(height - y + 8.),
        }
    } else {
        ClipMenuAnchor::Below { top: px(y + 8.) }
    };
    (left, anchor)
}

impl EditorWindow {
    pub(crate) fn transition_popover_index(&self) -> Option<usize> {
        self.transition_popover.map(|popover| popover.index)
    }

    fn effective_transition_at(&self, index: usize) -> Option<frameever_project::ClipTransition> {
        self.project
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.effective_transition(index))
    }

    fn cut_x(&self, index: usize, viewport_width: f32) -> Option<f32> {
        let clip = self.timeline.clips.get(index)?;
        let secs_per_pixel = self.secs_per_pixel(viewport_width);
        Some(
            timeline::content_left()
                + ((clip.start - self.view.transform.position) / secs_per_pixel) as f32,
        )
    }

    pub(super) fn close_stale_transition_popover(&mut self) {
        let Some(popover) = self.transition_popover else {
            return;
        };
        let still_there = self.project.timeline.as_ref().is_some_and(|timeline| {
            timeline.effective_transition(popover.index).is_some()
                && transition_clip_keys(timeline, popover.index) == Some(popover.clips)
        });
        if !still_there {
            self.transition_popover = None;
        }
    }

    pub(super) fn open_transition_popover(
        &mut self,
        index: usize,
        y: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(clips) = self.project.timeline.as_ref().and_then(|timeline| {
            timeline.effective_transition(index)?;
            transition_clip_keys(timeline, index)
        }) else {
            return;
        };
        let viewport = window.viewport_size();
        let x = self
            .cut_x(index, f32::from(viewport.width))
            .map_or(viewport.width / 2., px);
        let focus = self.focus_handle_for_menu();
        window.focus(&focus, cx);
        self.clip_speed = None;
        let (left, anchor) = popover_placement(point(x, y), viewport);
        self.transition_popover = Some(TransitionPopover {
            index,
            left,
            anchor,
            hovered: None,
            clips,
        });
        cx.notify();
    }

    pub(super) fn edit_transition_at(
        &mut self,
        index: usize,
        y: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.effective_transition_at(index).is_none() {
            self.set_clip_transition(
                index,
                Some((ClipTransitionType::CrossFade, DEFAULT_TRANSITION_SECONDS)),
                window,
                cx,
            );
        }
        self.open_transition_popover(index, y, window, cx);
    }

    pub(super) fn set_clip_transition(
        &mut self,
        index: usize,
        transition: Option<(ClipTransitionType, f64)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_project("clip-transition", window, cx, |project| {
            project.timeline.as_mut().is_some_and(|timeline| {
                crate::editor_clips::set_clip_transition(timeline, index, transition)
            })
        });
    }

    pub(super) fn choose_transition_kind(
        &mut self,
        index: usize,
        kind: ClipTransitionType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.effective_transition_at(index) else {
            return;
        };
        self.set_clip_transition(index, Some((kind, current.duration)), window, cx);
    }

    pub(super) fn remove_open_transition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(popover) = self.transition_popover.take() else {
            return;
        };
        self.set_clip_transition(popover.index, None, window, cx);
        cx.notify();
    }

    pub(super) fn press_clip_body(
        &mut self,
        index: usize,
        (start, end): (f64, f64),
        down_x: f32,
        (shift, multi): (bool, bool),
        press_time: f64,
        cx: &mut Context<Self>,
    ) {
        self.transition_drag_base = None;
        let transition_drag = self
            .project
            .timeline
            .as_ref()
            .filter(|timeline| {
                !shift && !multi && crate::editor_clips::can_add_transition(timeline, index)
            })
            .map(|timeline| {
                let current = timeline.effective_transition(index);
                DragKind::TransitionDuration {
                    index,
                    initial: current.map_or(0., |transition| transition.duration),
                    grow: -1.,
                    kind: current.map_or_else(Default::default, |transition| transition.kind),
                    from_body: true,
                }
            });
        self.drag = Some(Drag {
            track: TrackKind::Clip,
            index,
            kind: transition_drag.unwrap_or(DragKind::Move {
                start,
                end,
                bounds: DragBounds { min: 0., max: 0. },
            }),
            down_x,
            origin_x: transition_drag.map(|_| down_x),
            moved: false,
            threshold: 4.,
            selects_on_click: true,
            shift,
            multi,
            press_time,
            paused: false,
        });
        cx.notify();
    }

    pub(super) fn press_transition_edge(
        &mut self,
        index: usize,
        grow: f64,
        down_x: f32,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.effective_transition_at(index) else {
            return;
        };
        self.transition_drag_base = None;
        self.transition_popover = None;
        self.drag = Some(Drag {
            track: TrackKind::Clip,
            index,
            kind: DragKind::TransitionDuration {
                index,
                initial: current.duration,
                grow,
                kind: current.kind,
                from_body: false,
            },
            down_x,
            origin_x: Some(down_x),
            moved: false,
            threshold: 0.,
            selects_on_click: false,
            shift: false,
            multi: false,
            press_time: self.playhead,
            paused: false,
        });
        cx.notify();
    }

    pub(super) fn drag_transition(
        &mut self,
        index: usize,
        (initial, grow_by): (f64, f64),
        kind: ClipTransitionType,
        cx: &mut Context<Self>,
    ) {
        if self.transition_drag_base.is_none() {
            self.transition_drag_base = self.project.timeline.clone();
            self.history.pause();
            if let Some(drag) = self.drag.as_mut() {
                drag.paused = true;
            }
        }
        let Some(base) = self.transition_drag_base.as_ref() else {
            return;
        };
        let maximum = crate::editor_clips::max_transition_duration(base, index);
        let duration = crate::editor_clips::dragged_transition_duration(initial, grow_by, maximum);
        let current = self
            .effective_transition_at(index)
            .map(|transition| transition.duration);
        if current == duration {
            return;
        }
        let next = crate::editor_clips::apply_dragged_transition(base, index, kind, duration);
        let Some(timeline) = self.project.timeline.as_mut() else {
            return;
        };
        *timeline = next;
        self.synchronize_caption_track(false);
        self.history.record(&self.project);
        self.rebuild_timeline(cx);
        self.publish_project();
        cx.notify();
        if let Some(clip) = self.timeline.clips.get(index) {
            self.set_preview_time(clip.start + duration.unwrap_or(0.) / 2.);
        }
    }

    pub(super) fn finish_transition_drag(&mut self, moved: bool) {
        self.transition_drag_base = None;
        if moved {
            self.view.preview_time = None;
            self.emit_preview_frame();
        }
    }

    pub(super) fn render_transition_overlays(
        &self,
        model: &timeline::TimelineModel,
        viewport_width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = self.theme;
        let lane_width = timeline::content_width(viewport_width);
        let secs_per_pixel = self.view.transform.secs_per_pixel(lane_width);
        let position = self.view.transform.position;
        let open = self.transition_popover_index();
        let hovered = match self.hovered_segment {
            Some((TrackKind::Clip, _, index)) => Some(index),
            _ => None,
        };
        let quiet = self.drag.is_some() || self.split_mode;
        let ink = Hsla::from(theme.foreground);
        let mut layer = div()
            .absolute()
            .left(px(timeline::TRACK_GUTTER))
            .right_0()
            .top_0()
            .bottom_0()
            .overflow_hidden();
        let Some(project_timeline) = self.project.timeline.as_ref() else {
            return layer.into_any_element();
        };
        for (index, clip) in model.clips.iter().enumerate().skip(1) {
            let timeline::SegmentDetail::Clip { transition_in, .. } = &clip.detail else {
                continue;
            };
            let x = ((clip.start - position) / secs_per_pixel) as f32;
            match transition_in {
                Some(_) if self.split_mode => {}
                Some(seconds) => {
                    let Some(kind) = project_timeline
                        .effective_transition(index)
                        .map(|transition| transition.kind)
                    else {
                        continue;
                    };
                    let width = (*seconds / secs_per_pixel) as f32;
                    let band_width = width.max(TRANSITION_EDGE_PX * 2. + TRANSITION_DIAMOND_SIZE);
                    let left = x + (width - band_width) / 2.;
                    if left + band_width < 0. || left > lane_width {
                        continue;
                    }
                    let selected = open == Some(index);
                    let label = transition_label(kind);
                    let edge = |id: &'static str, grow: f64| {
                        div()
                            .id(gpui::ElementId::NamedInteger(id.into(), index as u64))
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .w(px(TRANSITION_EDGE_PX))
                            .when(grow < 0., |edge| edge.left_0())
                            .when(grow > 0., |edge| edge.right_0())
                            .cursor(gpui::CursorStyle::ResizeLeftRight)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.press_transition_edge(
                                        index,
                                        grow,
                                        f32::from(event.position.x),
                                        cx,
                                    );
                                }),
                            )
                    };
                    let mark_size = TRANSITION_DIAMOND_SIZE + 6.;
                    layer = layer.child(
                        div()
                            .absolute()
                            .left(px(left))
                            .w(px(band_width))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(!quiet || selected, |band| {
                                band.child(edge("clip-transition-start", -1.))
                                    .child(edge("clip-transition-end", 1.))
                            })
                            .child(
                                div()
                                    .id(gpui::ElementId::NamedInteger(
                                        "clip-transition".into(),
                                        index as u64,
                                    ))
                                    .size(px(mark_size))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .cursor_pointer()
                                    .when(selected, |mark| mark.border_1().border_color(ink))
                                    .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                                    .tooltip(move |_window, cx| {
                                        ui::Tooltip::new(&theme, label).view(cx)
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(
                                        move |this, event: &gpui::ClickEvent, window, cx| {
                                            if this.transition_popover_index() == Some(index) {
                                                this.transition_popover = None;
                                                cx.notify();
                                                return;
                                            }
                                            this.open_transition_popover(
                                                index,
                                                event.position().y,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(diamond(ink, TRANSITION_DIAMOND_SIZE)),
                            ),
                    );
                }
                None => {
                    if quiet
                        || !(hovered == Some(index) || hovered == Some(index - 1))
                        || !crate::editor_clips::can_add_transition(project_timeline, index)
                    {
                        continue;
                    }
                    let left = x - TRANSITION_ADD_SIZE / 2.;
                    if left + TRANSITION_ADD_SIZE < 0. || left > lane_width {
                        continue;
                    }
                    layer = layer.child(
                        div()
                            .absolute()
                            .left(px(left))
                            .w(px(TRANSITION_ADD_SIZE))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .id(gpui::ElementId::NamedInteger(
                                        "clip-transition-add".into(),
                                        index as u64,
                                    ))
                                    .size(px(TRANSITION_ADD_SIZE))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .tab_index(0)
                                    .cursor_pointer()
                                    .bg(ink)
                                    .tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                                    .tooltip(move |_window, cx| {
                                        ui::Tooltip::new(&theme, t!("Add transition")).view(cx)
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(
                                        move |this, event: &gpui::ClickEvent, window, cx| {
                                            this.edit_transition_at(
                                                index,
                                                event.position().y,
                                                window,
                                                cx,
                                            );
                                        },
                                    ))
                                    .child(
                                        svg()
                                            .path("icons/plus.svg")
                                            .size(px(12.))
                                            .text_color(Hsla::from(theme.background)),
                                    ),
                            ),
                    );
                }
            }
        }
        layer.into_any_element()
    }

    pub(super) fn render_clip_menu_transition_row(
        &self,
        index: usize,
        menu_anchor: ClipMenuAnchor,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let theme = self.theme;
        let timeline = self.project.timeline.as_ref()?;
        if index == 0 || index >= timeline.segments.len() {
            return None;
        }
        let current = timeline.effective_transition(index);
        let enabled = current.is_some() || crate::editor_clips::can_add_transition(timeline, index);
        let value = current.map_or(t!("None"), |transition| transition_label(transition.kind));
        let viewport_height = window.viewport_size().height;
        let anchor_y = match menu_anchor {
            ClipMenuAnchor::Above { bottom } => viewport_height - bottom + px(8.),
            ClipMenuAnchor::Below { top } => top - px(8.),
        };
        let muted = Hsla::from(theme.editor.text_3);
        Some(
            div()
                .id("clip-menu-transition")
                .h(px(32.))
                .w_full()
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(8.))
                .px(px(8.))
                .text_size(px(12.5))
                .text_color(Hsla::from(if enabled {
                    theme.editor.text_1
                } else {
                    theme.editor.text_3
                }))
                .child(
                    div()
                        .size(px(14.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(diamond(
                            Hsla::from(if enabled {
                                theme.editor.text_2
                            } else {
                                theme.editor.text_3
                            }),
                            8.,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(transition_title()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.5))
                        .text_color(muted)
                        .child(value),
                )
                .child(
                    svg()
                        .path("icons/chevron-right.svg")
                        .size(px(14.))
                        .flex_none()
                        .text_color(muted),
                )
                .when(enabled, |row| {
                    row.tab_index(0)
                        .cursor_pointer()
                        .hover(|this| this.bg(Hsla::from(theme.editor.ctl_hover)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.clip_speed = None;
                            this.edit_transition_at(index, anchor_y, window, cx);
                        }))
                })
                .when(!enabled, |row| {
                    row.tooltip_show_delay(ui::TOOLTIP_SHOW_DELAY)
                        .tooltip(move |_window, cx| {
                            ui::Tooltip::new(&theme, t!("Clip is too short for a transition."))
                                .view(cx)
                        })
                })
                .into_any_element(),
        )
    }

    pub(super) fn render_transition_popover(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let popover = self.transition_popover?;
        let theme = self.theme;
        let index = popover.index;
        let current = self.effective_transition_at(index)?;

        let tile = |kind: ClipTransitionType| {
            let selected = current.kind == kind;
            let preview = if popover.hovered == Some(kind) {
                div()
                    .child(transition_thumbnail(&theme, kind, 0.))
                    .with_animation(
                        gpui::ElementId::NamedInteger("transition-preview".into(), kind as u64),
                        Animation::new(PREVIEW_LOOP).repeat(),
                        move |_, delta| {
                            div().child(transition_thumbnail(&theme, kind, preview_progress(delta)))
                        },
                    )
                    .into_any_element()
            } else {
                transition_thumbnail(&theme, kind, 0.4).into_any_element()
            };
            div()
                .id(gpui::ElementId::NamedInteger(
                    "transition-kind".into(),
                    kind as u64,
                ))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .min_h(px(44.))
                .px(px(4.))
                .py(px(6.))
                .rounded(px(6.))
                .border_2()
                .tab_index(0)
                .cursor_pointer()
                .border_color(if selected {
                    Hsla::from(theme.foreground)
                } else {
                    gpui::transparent_black()
                })
                .bg(if selected {
                    theme.tile_selected_bg()
                } else {
                    Hsla::from(theme.muted)
                })
                .when(!selected, |tile| {
                    tile.hover(|tile| tile.bg(Hsla::from(theme.editor.ctl_hover)))
                })
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if let Some(popover) = this.transition_popover.as_mut() {
                        if *hovered {
                            popover.hovered = Some(kind);
                        } else if popover.hovered == Some(kind) {
                            popover.hovered = None;
                        }
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_transition_kind(index, kind, window, cx);
                }))
                .child(preview)
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(11.))
                        .text_color(Hsla::from(if selected {
                            theme.editor.text_1
                        } else {
                            theme.editor.text_2
                        }))
                        .child(transition_label(kind)),
                )
        };

        let mut card = div()
            .id("transition-popover")
            .absolute()
            .left(popover.left)
            .w(px(TRANSITION_POPOVER_WIDTH))
            .flex()
            .flex_col()
            .gap(px(4.))
            .rounded(px(12.))
            .border_1()
            .border_color(Hsla::from(theme.editor.line))
            .bg(Hsla::from(theme.editor.card))
            .p(px(4.))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            );
        card = match popover.anchor {
            ClipMenuAnchor::Above { bottom } => card.bottom(bottom),
            ClipMenuAnchor::Below { top } => card.top(top),
        };
        let card = card
            .child(
                div()
                    .px(px(10.))
                    .pt(px(6.))
                    .pb(px(2.))
                    .text_size(px(11.))
                    .text_color(Hsla::from(theme.editor.text_2))
                    .child(transition_title()),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(4)
                    .gap(px(6.))
                    .px(px(4.))
                    .pt(px(2.))
                    .pb(px(4.))
                    .children(ClipTransitionType::ALL.into_iter().map(tile)),
            )
            .child(div().px(px(4.)).child(self.slider_field(
                t!("Duration"),
                crate::editor_sidebar::SliderKey::ClipTransition(index),
                "secs",
                cx,
            )))
            .child(div().h(px(1.)).bg(Hsla::from(theme.editor.line)))
            .child(
                clip_menu_row(
                    &theme,
                    "transition-remove",
                    "icons/trash.svg",
                    t!("Remove transition"),
                    ClipMenuTrailing::None,
                    true,
                    true,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.remove_open_transition(window, cx);
                })),
            );

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(
                    div()
                        .id("transition-popover-backdrop")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.transition_popover = None;
                            cx.notify();
                        })),
                )
                .child(card)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(
        cx: &mut gpui::TestAppContext,
        transitions: serde_json::Value,
    ) -> (
        Entity<EditorWindow>,
        &mut gpui::VisualTestContext,
        tempfile::TempDir,
    ) {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().to_path_buf();
        let (editor, cx) =
            cx.add_window_view(move |window, cx| EditorWindow::new(path.clone(), window, cx));
        editor.update_in(cx, |editor, window, cx| {
            editor.set_error("transition fixture".into(), window, cx);
            editor.project.timeline = Some(
                serde_json::from_value(serde_json::json!({
                    "segments": [
                        {"recordingSegment": 0, "timescale": 1.0, "start": 0.0, "end": 10.0},
                        {"recordingSegment": 0, "timescale": 1.0, "start": 10.0, "end": 20.0}
                    ],
                    "zoomSegments": [],
                    "transitions": transitions
                }))
                .unwrap(),
            );
            editor.history.record(&editor.project);
            editor.rebuild_timeline(cx);
        });
        (editor, cx, project)
    }

    fn transition(editor: &EditorWindow) -> Option<(ClipTransitionType, f64)> {
        editor
            .effective_transition_at(1)
            .map(|transition| (transition.kind, transition.duration))
    }

    fn pixels_for(editor: &EditorWindow, window: &Window, seconds: f64) -> f32 {
        (seconds / editor.secs_per_pixel(f32::from(window.viewport_size().width))) as f32
    }

    #[gpui::test]
    fn dragging_a_clip_onto_the_one_before_makes_a_transition_one_undo_removes(
        cx: &mut gpui::TestAppContext,
    ) {
        let (editor, cx, _project) = fixture(cx, serde_json::json!([]));
        editor.update_in(cx, |editor, window, cx| {
            let travel = pixels_for(editor, window, 1.0);
            editor.press_clip_body(1, (10., 20.), 600., (false, false), 10., cx);
            for step in 1..=4 {
                editor.drag_mouse_move(600. - travel * step as f32 / 4., true, window, cx);
            }
            editor.drag_mouse_up(window, cx);
            let (kind, duration) = transition(editor).expect("a transition");
            assert_eq!(kind, ClipTransitionType::CrossFade);
            assert!((duration - 1.0).abs() < 0.05, "{duration}");
            assert!(editor.transition_drag_base.is_none());
            editor.undo(window, cx);
            assert_eq!(transition(editor), None);
        });
    }

    #[gpui::test]
    fn a_first_move_right_without_a_transition_only_selects(cx: &mut gpui::TestAppContext) {
        let (editor, cx, _project) = fixture(cx, serde_json::json!([]));
        editor.update_in(cx, |editor, window, cx| {
            editor.press_clip_body(1, (10., 20.), 600., (false, false), 12., cx);
            editor.drag_mouse_move(680., true, window, cx);
            editor.drag_mouse_up(window, cx);
            assert_eq!(transition(editor), None);
            assert!(
                editor
                    .selection
                    .as_ref()
                    .is_some_and(|selection| selection.contains(TrackKind::Clip, 1))
            );
        });
    }

    #[gpui::test]
    fn an_edge_drag_resizes_and_dragging_past_half_the_minimum_removes(
        cx: &mut gpui::TestAppContext,
    ) {
        let (editor, cx, _project) = fixture(
            cx,
            serde_json::json!([{ "segmentIndex": 1, "type": "cross-fade", "effect": "wipe-up", "duration": 1.0 }]),
        );
        editor.update_in(cx, |editor, window, cx| {
            let half_second = pixels_for(editor, window, 0.5);
            editor.press_transition_edge(1, 1., 500., cx);
            editor.drag_mouse_move(500. + half_second, true, window, cx);
            editor.drag_mouse_up(window, cx);
            let (kind, duration) = transition(editor).unwrap();
            assert_eq!(kind, ClipTransitionType::WipeUp);
            assert!((duration - 1.5).abs() < 0.05, "{duration}");

            let back = pixels_for(editor, window, 1.49);
            editor.press_transition_edge(1, 1., 500., cx);
            editor.drag_mouse_move(500. - back, true, window, cx);
            editor.drag_mouse_up(window, cx);
            assert_eq!(transition(editor), None);
            editor.undo(window, cx);
            assert_eq!(transition(editor).map(|(kind, _)| kind), Some(kind));
        });
    }

    #[gpui::test]
    fn the_popover_follows_its_cut_and_closes_when_the_clips_are_renumbered(
        cx: &mut gpui::TestAppContext,
    ) {
        let (editor, cx, _project) = fixture(
            cx,
            serde_json::json!([{ "segmentIndex": 1, "type": "fade-through-black", "duration": 0.5 }]),
        );
        editor.update_in(cx, |editor, window, cx| {
            editor.open_transition_popover(0, px(600.), window, cx);
            assert_eq!(editor.transition_popover_index(), None);
            editor.open_transition_popover(1, px(600.), window, cx);
            assert_eq!(editor.transition_popover_index(), Some(1));
            editor.edit_live(
                |timeline| {
                    timeline.zoom_segments = serde_json::from_value(serde_json::json!([
                        {"start": 2.0, "end": 3.0, "amount": 1.5, "mode": "auto"}
                    ]))
                    .unwrap();
                    true
                },
                cx,
            );
            assert_eq!(editor.transition_popover_index(), Some(1));
            editor.edit_live(
                |timeline| edits::split_clip_segment(timeline, 4.0, None),
                cx,
            );
            assert_eq!(editor.transition_popover_index(), None);
        });
    }

    #[gpui::test]
    fn the_popover_opens_inside_the_window_above_the_cut(cx: &mut gpui::TestAppContext) {
        let (editor, cx, _project) = fixture(
            cx,
            serde_json::json!([{ "segmentIndex": 1, "type": "push-left", "duration": 0.5 }]),
        );
        editor.update_in(cx, |editor, window, cx| {
            let viewport = window.viewport_size();
            editor.open_transition_popover(1, viewport.height - px(100.), window, cx);
            let popover = editor.transition_popover.unwrap();
            assert!(popover.left >= px(12.));
            assert!(popover.left + px(TRANSITION_POPOVER_WIDTH) <= viewport.width - px(12.));
            assert!(matches!(popover.anchor, ClipMenuAnchor::Above { .. }));
            assert!(editor.render_transition_popover(cx).is_some());
        });
    }

    #[test]
    fn each_kind_has_its_own_label() {
        let labels: std::collections::HashSet<_> = ClipTransitionType::ALL
            .into_iter()
            .map(transition_label)
            .collect();
        assert_eq!(labels.len(), ClipTransitionType::ALL.len());
    }

    #[test]
    fn the_preview_holds_each_side_before_and_after_the_transition() {
        assert_eq!(preview_progress(0.0), 0.0);
        assert_eq!(preview_progress(0.2), 0.0);
        assert!((preview_progress(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(preview_progress(0.8), 1.0);
        assert_eq!(preview_progress(1.0), 1.0);
    }
}
