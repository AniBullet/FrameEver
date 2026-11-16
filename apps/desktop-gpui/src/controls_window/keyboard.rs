#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Microphone,
    Pause,
    Restart,
    Issue,
    Delete,
    ChapterMark,
    Draw,
    Options,
    Stop,
    DrawTool(usize),
    DrawColour(usize),
    DrawUndo,
    DrawClear,
    PickMicrophone,
    MicrophoneChoice(usize),
    PickCamera,
    CameraChoice(usize),
    CameraPreview,
    HideControls,
    CancelAction,
    AcceptAction,
    DismissIssue,
    IssueDetails,
    ExportLogs,
}

pub(super) type Group = Vec<(Control, bool)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Previous,
    Next,
    First,
    Last,
    NextGroup,
    PreviousGroup,
    Press,
    Escape,
}

pub(super) fn command(key: &str, shift: bool) -> Option<Command> {
    Some(match key {
        "left" | "up" => Command::Previous,
        "right" | "down" => Command::Next,
        "home" => Command::First,
        "end" => Command::Last,
        "tab" if shift => Command::PreviousGroup,
        "tab" => Command::NextGroup,
        "enter" | "space" => Command::Press,
        "escape" => Command::Escape,
        _ => return None,
    })
}

fn enabled(group: &Group) -> impl DoubleEndedIterator<Item = Control> + '_ {
    group
        .iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(control, _)| *control)
}

fn group_of(groups: &[Group], control: Control) -> Option<usize> {
    groups
        .iter()
        .position(|group| enabled(group).any(|candidate| candidate == control))
}

pub(super) fn resolve(
    groups: &[Group],
    current: Option<Control>,
    fallback: Option<Control>,
) -> Option<Control> {
    [current, fallback]
        .into_iter()
        .flatten()
        .find(|control| group_of(groups, *control).is_some())
        .or_else(|| groups.iter().rev().find_map(|group| enabled(group).next()))
}

pub(super) fn moved(groups: &[Group], current: Control, command: Command) -> Control {
    let Some(at) = group_of(groups, current) else {
        return current;
    };
    let group: Vec<Control> = enabled(&groups[at]).collect();
    let index = group
        .iter()
        .position(|control| *control == current)
        .unwrap_or_default();
    match command {
        Command::Previous => group[index.saturating_sub(1)],
        Command::Next => group[(index + 1).min(group.len() - 1)],
        Command::First => group[0],
        Command::Last => group[group.len() - 1],
        Command::NextGroup | Command::PreviousGroup => {
            let count = groups.len();
            let forward = command == Command::NextGroup;
            (1..count)
                .map(|step| {
                    if forward {
                        (at + step) % count
                    } else {
                        (at + count - step) % count
                    }
                })
                .find_map(|other| enabled(&groups[other]).next())
                .unwrap_or(current)
        }
        Command::Press | Command::Escape => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> Group {
        vec![
            (Control::Microphone, false),
            (Control::Pause, true),
            (Control::Restart, true),
            (Control::Delete, false),
            (Control::Options, true),
            (Control::Stop, true),
        ]
    }

    fn card() -> Group {
        vec![(Control::CancelAction, true), (Control::AcceptAction, true)]
    }

    #[test]
    fn keys_follow_the_toolbar_pattern() {
        assert_eq!(command("left", false), Some(Command::Previous));
        assert_eq!(command("right", false), Some(Command::Next));
        assert_eq!(command("home", false), Some(Command::First));
        assert_eq!(command("end", false), Some(Command::Last));
        assert_eq!(command("enter", false), Some(Command::Press));
        assert_eq!(command("space", false), Some(Command::Press));
        assert_eq!(command("escape", false), Some(Command::Escape));
        assert_eq!(command("tab", false), Some(Command::NextGroup));
        assert_eq!(command("tab", true), Some(Command::PreviousGroup));
        assert_eq!(command("a", false), None);
    }

    #[test]
    fn arrows_skip_disabled_controls_and_stop_at_the_ends() {
        let groups = vec![bar()];
        assert_eq!(
            moved(&groups, Control::Restart, Command::Next),
            Control::Options
        );
        assert_eq!(moved(&groups, Control::Stop, Command::Next), Control::Stop);
        assert_eq!(
            moved(&groups, Control::Pause, Command::Previous),
            Control::Pause
        );
        assert_eq!(
            moved(&groups, Control::Stop, Command::First),
            Control::Pause
        );
        assert_eq!(moved(&groups, Control::Pause, Command::Last), Control::Stop);
    }

    #[test]
    fn tab_moves_between_the_open_card_and_the_bar() {
        let groups = vec![card(), bar()];
        assert_eq!(
            moved(&groups, Control::Restart, Command::NextGroup),
            Control::CancelAction
        );
        assert_eq!(
            moved(&groups, Control::AcceptAction, Command::PreviousGroup),
            Control::Pause
        );
        let groups = vec![bar()];
        assert_eq!(
            moved(&groups, Control::Restart, Command::NextGroup),
            Control::Restart
        );
    }

    #[test]
    fn focus_lands_on_the_first_bar_control_and_survives_closing_a_card() {
        let groups = vec![card(), bar()];
        assert_eq!(resolve(&groups, None, None), Some(Control::Pause));
        assert_eq!(
            resolve(&groups, Some(Control::AcceptAction), None),
            Some(Control::AcceptAction)
        );
        let groups = vec![bar()];
        assert_eq!(
            resolve(&groups, Some(Control::AcceptAction), Some(Control::Restart)),
            Some(Control::Restart)
        );
        assert_eq!(
            resolve(&groups, Some(Control::Delete), None),
            Some(Control::Pause)
        );
        assert_eq!(resolve(&[], None, None), None);
    }
}
