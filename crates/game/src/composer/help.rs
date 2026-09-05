//! How to use the editor, in the editor.
//!
//! The composer's whole premise is that a scientist can change a behavioural
//! assumption without writing Rust. That premise fails quietly if the only
//! documentation is a paragraph in a repository they have never opened — and it
//! is the *quiet* part that matters: someone who cannot find how to connect two
//! boxes does not file a bug, they conclude the tool does not do that.
//!
//! So the help is here, next to the thing it is about, and it is written as
//! answers to the questions the editor actually raises rather than as a tour of
//! the interface. Every section is collapsed by default except the first, and
//! the first is the one nobody can guess: what a graph in here *is*.
//!
//! The other half of this tab is the file list. A behaviour library is a folder
//! of small JSON files meant to be shared and diffed, and the two things a
//! person needs from it are "what is on disk" and "why did that one not load".

use bevy_egui::egui;

use behavior::Domain;

use super::Composer;

pub fn panel(ui: &mut egui::Ui, c: &mut Composer) {
    ui.heading("Help");
    ui.small("Everything below is about the kind of agent you have open, which is currently:");
    ui.horizontal(|ui| {
        ui.strong(c.domain().label());
        ui.small(format!("— {}", c.domain().agent_label()));
    });
    ui.separator();

    section(ui, "Start here: change a behaviour without coding", true, |ui| {
        bullet(ui, "In Guided settings, choose the kind of agent and its behaviour.");
        bullet(ui, "Choose a profile, then a rule and Adjust settings. Shared settings changes the defaults used by profiles without their own setting.");
        bullet(ui, "Check Profiles: a population profile needs a positive share; a unit profile must be enabled. A behaviour without an active profile affects no agents.");
        bullet(ui, "Use Try a situation to preview a decision without changing the incident.");
        bullet(ui, "Apply and restart runs your changes from the beginning of the incident. Save to disk also keeps them for the next desktop launch.");
        bullet(ui, "Select an agent and open Live debugger. Read why it acts, then press Next decision to advance the incident.");
        para(ui, "Advanced wiring lets you add rules and change their connections. You can adjust existing rules without using it.");
    });

    section(ui, "What a behaviour is", false, |ui| {
        para(
            ui,
            "A behaviour is a graph of boxes and wires that answers one question about one \
             agent, over and over. It is **not** a flowchart and there is no token moving \
             through it: every box is evaluated on every decision tick, values flow left to \
             right along the wires, and the answer is whichever action proposal comes out \
             strongest.",
        );
        para(
            ui,
            "That matters for reading it. A box being \"reached\" is not a thing — every box \
             always runs. What varies is whether what it produced ended up mattering, which \
             is exactly what the Live tab colours in.",
        );
        para(ui, &format!("A graph of this kind decides: **{}**.", actions_of(c.domain())));
        para(
            ui,
            "What it cannot decide is where the agent goes on the map, how fast they move, \
             or whether they survive. Those are the model's, and keeping the line there is \
             what makes a behaviour somebody wrote safe to run without review.",
        );
    });

    section(ui, "Adding and configuring a node", false, |ui| {
        bullet(ui, "Click an entry in the Palette on the left. It lands on the canvas.");
        bullet(ui, "Or right-click empty canvas to place one exactly where you clicked.");
        bullet(
            ui,
            "Or drag a wire from a pin into empty space: the menu that opens offers only \
             the nodes that can accept what you dragged.",
        );
        ui.add_space(4.0);
        para(
            ui,
            "Select a node — click it, or pick Inspect from its right-click menu — and the \
             **Node** tab shows what it does and every number it turns on. Hover any \
             parameter's label for what that number means.",
        );
        para(
            ui,
            "**Start with Blocks.** A block is one whole assumption in one box — \"how much \
             alarm before they act\" — reading what it needs off the agent itself and putting \
             the numbers on the front. The primitives underneath are all still in the \
             palette, and rebuilding a block out of them is the supported way to change its \
             *structure* rather than its numbers. If your behaviour is thirty boxes, there \
             is probably a block for most of it.",
        );
    });

    section(ui, "Connecting nodes", false, |ui| {
        para(ui, "Drag from an output pin on the right of a box to an input pin on the left of another.");
        para(
            ui,
            "**A wire that will not form is telling you something.** Pins carry types — \
             number, condition, plan, action — and two pins only connect if the types match. \
             There is no coercion, deliberately: a condition silently turning into 1.0 would \
             let a graph that reads wrong run fine.",
        );
        ui.add_space(4.0);
        bullet(ui, "Circle = number. A magnitude; most are 0–1, and the ones that are not say so.");
        bullet(ui, "Square = condition. True or false.");
        bullet(ui, "Triangle = plan. A household's stated intent.");
        bullet(ui, "Star = action. A proposal, carrying its priority.");
        ui.add_space(4.0);
        para(
            ui,
            "A hollow pin is an unconnected input that has a default, so the node still runs. \
             A solid one is carrying something. Right-click a wire to remove it.",
        );
    });

    section(ui, "Conditions, and how the decision is made", false, |ui| {
        para(
            ui,
            "Every branch ends in an action node with a condition wired into its `when` \
             input. While the condition holds, the node proposes its action at the priority \
             you gave it; while it does not, it emits a withheld proposal so the trace can \
             show the branch was checked and declined.",
        );
        para(
            ui,
            "All the proposals go into the one decision sink, which takes as many wires as \
             you like and picks the **strongest**. That is what lets you add a branch without \
             thinking about the ones already there: it either outbids them or it does not.",
        );
        para(
            ui,
            "Priorities are parameters, so they are the first thing to try changing. The \
             shipped orderings encode what the model insists on — sheltering outbids running, \
             because on a cut road the running is what kills people — and inverting one is a \
             fast way to find out why it was that way round.",
        );
        para(
            ui,
            "With nothing firing at all the agent does its domain's default, which is a real \
             answer rather than a missing one.",
        );
    });

    section(ui, "Profiles: one graph, many kinds of agent", false, |ui| {
        para(
            ui,
            "A **profile** is this graph plus a flat list of parameter overrides plus some \
             starting traits. There is no inheritance and no parent, on purpose: \"why did \
             this agent do that\" is answered by reading one file.",
        );
        para(
            ui,
            "Select a profile in the **Profiles** tab and the Node tab starts editing that \
             profile's override rather than the graph's own value — and says which of the two \
             it is writing to, in purple. Set the profile to \"none\" to edit the graph again.",
        );
        para(ui, &format!("These profiles are assigned {}.", assignment_of(c.domain())));
    });

    section(ui, "Running and debugging", false, |ui| {
        para(
            ui,
            "**Bench** puts a made-up agent in a situation and reads the answer back node by \
             node. The situations that ship are moments the hand-written model either handled \
             or visibly did not; every field is editable underneath. The **Sweep** view varies \
             one field across its range and reports where the decision actually changes — a \
             threshold in a graph like this is never one number, so the alternative is \
             guessing.",
        );
        para(
            ui,
            "**Apply and restart** rebuilds the agent model on this library and replays the \
             incident from the beginning. The fire, the weather, the seed and the ignition \
             list are untouched, so it is a like-for-like comparison rather than a new roll \
             of the dice.",
        );
        ui.add_space(4.0);
        para(
            ui,
            "**Live** is the other half. Click an agent on the map and this canvas starts \
             showing what their behaviour is doing: every node's value on the box, the path \
             that produced the decision picked out, and the branches that were checked and \
             declined told apart from the ones nothing reached. `⏭` in that tab advances \
             exactly one decision tick, so you can walk a decision forward and watch where it \
             turns.",
        );
        para(
            ui,
            "Two things it cannot do, and it is better to know than to wonder. It shows the \
             behaviour **as applied** — edits since the last Apply are not what is running, \
             and it says so. And it does not claim which arm of an `Or` was the one that \
             mattered: the whole path that fed the decision is lit, because a highlight that \
             guessed would be worse than no highlight.",
        );
    });

    section(ui, "Saving, loading and sharing", false, |ui| {
        para(
            ui,
            "**Save** writes the whole library to disk as one small JSON file per behaviour \
             and per profile. One file per thing, so a changed threshold is a three-line patch \
             a colleague can read, and two people editing different profiles do not conflict.",
        );
        para(
            ui,
            "**Reload** throws away every unsaved edit and re-reads the folder. Saving does \
             not apply and applying does not save — they are separate on purpose, because \
             \"try this\" and \"keep this\" are different intentions.",
        );
        ui.add_space(4.0);
        files(ui, c);
    });

    section(ui, "Keys", false, |ui| {
        for (key, what) in [
            ("g", "open and close this window"),
            ("space", "run / pause the incident"),
            ("[ ]", "slower / faster"),
            ("r", "restart the incident"),
            ("esc", "put down whatever map tool is armed"),
        ] {
            ui.horizontal(|ui| {
                ui.monospace(key);
                ui.small(what);
            });
        }
        ui.add_space(4.0);
        ui.small(
            "Shortcuts do nothing while a text field here has focus, which is why typing a \
             name does not restart the incident.",
        );
    });
}

/// What is on disk, what loaded, and what did not.
fn files(ui: &mut egui::Ui, c: &mut Composer) {
    ui.strong("Files on disk");
    ui.small(c.root.display().to_string());

    let bad = c.load_report.iter().filter(|f| !f.ok()).count();
    if c.load_report.is_empty() {
        ui.small("Nothing has been saved here yet — the shipped library is in memory.");
    } else {
        egui::ScrollArea::vertical().max_height(160.0).id_source("library-files").show(ui, |ui| {
            for f in &c.load_report {
                ui.horizontal_wrapped(|ui| {
                    if f.ok() {
                        ui.colored_label(egui::Color32::from_rgb(0x7a, 0xb2, 0x8a), "✔");
                        ui.small(f.name());
                        if let Some(id) = &f.id {
                            ui.small(format!("· {id}"));
                        }
                    } else {
                        // Named, with its parse error, next to the ones that
                        // worked. A file skipped in silence is an edit lost in
                        // silence.
                        ui.colored_label(egui::Color32::from_rgb(0xe0, 0x6c, 0x5f), "✖");
                        ui.small(f.name());
                    }
                });
                if let Some(e) = &f.error {
                    ui.small(egui::RichText::new(e).color(egui::Color32::from_rgb(0xe0, 0x6c, 0x5f)));
                }
            }
        });
        if bad > 0 {
            ui.small(format!(
                "{bad} file(s) were skipped. The rest of the library loaded; fix them and \
                 press Reload."
            ));
        }
    }

    // --- import and export ---------------------------------------------------
    ui.add_space(6.0);
    ui.strong("Import and export");
    ui.small(
        "For a behaviour someone sent you, or one you want to send. Import never overwrites: \
         an id already in the library is given a free one and the status line says so.",
    );
    ui.horizontal(|ui| {
        ui.label("Path");
        ui.add(
            egui::TextEdit::singleline(&mut c.transfer_path)
                .hint_text("~/somewhere/my-behaviour.json")
                .desired_width(f32::INFINITY),
        );
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Import")
            .on_hover_text("Read one behaviour or profile from that path into the library")
            .clicked()
        {
            let p = c.transfer_path.clone();
            c.import(&p);
        }
        if ui
            .button("Export behaviour")
            .on_hover_text("Write the open behaviour to that path")
            .clicked()
        {
            let p = c.transfer_path.clone();
            c.export(&p, true);
        }
        let has_profile = c.subtype.is_some();
        if ui
            .add_enabled(has_profile, egui::Button::new("Export profile"))
            .on_hover_text("Write the selected profile to that path")
            .clicked()
        {
            let p = c.transfer_path.clone();
            c.export(&p, false);
        }
    });
}

/// What this domain's action set actually is, spelled out rather than left to
/// the palette. It is the first thing anyone needs and the last thing a node
/// list makes obvious.
fn actions_of(d: Domain) -> &'static str {
    match d {
        Domain::Household => {
            "when a household starts getting ready, leaves immediately, commits to defending \
             the property, or shelters where it is"
        }
        Domain::Person => {
            "whether someone away from their household keeps walking out, stops and shelters, \
             or turns round for the house"
        }
        Domain::SuppressionUnit => {
            "when a crew, engine or aircraft breaks off — pulls back for its own safety, goes \
             for water, holds where it is, or returns to staging"
        }
    }
}

fn assignment_of(d: Domain) -> &'static str {
    match d {
        Domain::Household => {
            "by **share**: a relative weight, normalised, hashed onto the 750 anonymous \
             families. Zero keeps the profile and takes it out of play"
        }
        Domain::Person => {
            "by **share**, the same way households are — there are hundreds of them and no one \
             of them is a named individual"
        }
        Domain::SuppressionUnit => {
            "by **kind**, with an on/off switch. There are eight units and they are named \
             individuals, so a hash would make \"why did Autobotte 2 do that\" a question \
             about arithmetic rather than about a file"
        }
    }
}

fn section(
    ui: &mut egui::Ui,
    title: &str,
    open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    egui::CollapsingHeader::new(title).default_open(open).show(ui, body);
    ui.add_space(2.0);
}

/// A paragraph, with `**bold**` honoured because the alternative is either no
/// emphasis at all or a markdown dependency for six words.
fn para(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, part) in text.split("**").enumerate() {
            if part.is_empty() {
                continue;
            }
            if i % 2 == 1 {
                ui.label(egui::RichText::new(part).strong());
            } else {
                ui.label(part);
            }
        }
    });
    ui.add_space(4.0);
}

fn bullet(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label("  •");
        ui.label(text);
    });
}
