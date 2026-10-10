use super::{AgentSelection, detected_agents};
use crate::i18n::t;
use gpui_kit::component::{
    ActiveTheme as _, IndexPath, Sizable as _,
    input::{Input, InputEvent, InputState},
    searchable_list::SearchableVec,
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// Retained credential field. Discovery never changes the user's selection.
pub struct AgentPicker {
    select: Entity<SelectState<SearchableVec<SharedString>>>,
    path: Entity<InputState>,
    choices: Vec<Option<AgentSelection>>,
    labels: Vec<SharedString>,
    custom: bool,
    selected: AgentSelection,
    error: Option<SharedString>,
    loading: bool,
    _subscriptions: Vec<Subscription>,
    _scan: Task<()>,
}
impl AgentPicker {
    pub fn new(selected: AgentSelection, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let custom = false;
        let (choices, labels) = Self::options(&[], &selected);
        let selected_ix = choices
            .iter()
            .position(|item| item.as_ref() == Some(&selected))
            .unwrap_or(0);
        let select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels.clone()),
                Some(IndexPath::new(selected_ix)),
                window,
                cx,
            )
            .searchable(true)
        });
        let path = cx.new(|cx| {
            InputState::new(window, cx).default_value(match &selected {
                AgentSelection::Path(path) => path.to_string_lossy().into_owned(),
                _ => String::new(),
            })
        });
        let subscriptions = vec![
            cx.subscribe(
                &select,
                |this, _, event: &SelectEvent<SearchableVec<SharedString>>, cx| {
                    let SelectEvent::Confirm(Some(value)) = event else {
                        return;
                    };
                    let Some(ix) = this.labels.iter().position(|label| label == value) else {
                        return;
                    };
                    this.custom = ix == this.choices.len() - 1;
                    this.error = None;
                    if !this.custom {
                        this.selected = this.choices[ix].clone().expect("non-custom choice");
                    }
                    cx.notify();
                },
            ),
            cx.subscribe(&path, |this, _, event: &InputEvent, cx| {
                if this.custom && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                {
                    match this.value(cx) {
                        Ok(value) => {
                            this.selected = value.clone();
                            this.error = None;
                        }
                        Err(error) => this.error = Some(error),
                    }
                    cx.notify();
                }
            }),
        ];
        let scan = cx.spawn_in(window, async move |this, cx| {
            let agents = cx.background_spawn(async { detected_agents() }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let (choices, labels) = Self::options(&agents, &this.selected);
                // Preserve a custom draft while discovery finishes.
                let ix = if this.custom {
                    choices.len() - 1
                } else {
                    choices
                        .iter()
                        .position(|item| item.as_ref() == Some(&this.selected))
                        .unwrap_or(0)
                };
                this.choices = choices;
                this.labels = labels.clone();
                this.select.update(cx, |select, cx| {
                    select.set_items(SearchableVec::new(labels), window, cx);
                    select.set_selected_index(Some(IndexPath::new(ix)), window, cx);
                });
                this.loading = false;
                cx.notify();
            });
        });
        Self {
            select,
            path,
            choices,
            labels,
            custom,
            selected,
            error: None,
            loading: true,
            _subscriptions: subscriptions,
            _scan: scan,
        }
    }

    fn options(
        detected: &[(String, std::path::PathBuf)],
        selected: &AgentSelection,
    ) -> (Vec<Option<AgentSelection>>, Vec<SharedString>) {
        let mut choices = Vec::new();
        let mut labels = Vec::new();
        choices.extend([
            Some(AgentSelection::Auto),
            Some(AgentSelection::Environment),
        ]);
        labels.extend([t!("agent.auto"), t!("agent.environment")]);
        for (name, path) in detected {
            choices.push(Some(AgentSelection::Path(path.clone())));
            labels.push(format!("{name} — {}", path.display()).into());
        }
        if let AgentSelection::Path(path) = selected
            && !choices.contains(&Some(selected.clone()))
        {
            choices.push(Some(selected.clone()));
            labels.push(t!("agent.saved_path", path = path.display()));
        }
        choices.push(None); // The custom path editor.
        labels.push(t!("agent.custom"));
        (choices, labels)
    }

    pub fn value(&self, cx: &App) -> Result<AgentSelection, SharedString> {
        if self.custom {
            AgentSelection::custom(self.path.read(cx).value().as_ref())
        } else {
            Ok(self.selected.clone())
        }
    }
}
impl Render for AgentPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_2()
            .child(
                Select::new(&self.select)
                    .id("agent-choice")
                    .w_full()
                    .small(),
            )
            .when(self.custom, |view| {
                view.child(Input::new(&self.path).id("agent-path").small())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("agent.path_help")),
                    )
            })
            .when(self.loading, |view| {
                view.child(div().text_xs().child(t!("agent.detecting")))
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .id("agent-error")
                        .test_support()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
    }
}
