use crate::core::{ABS_RX, ABS_RY, ABS_RZ, ABS_X, ABS_Y, ABS_Z, EV_KEY};

/// Unified event passed through the plugin pipeline.
#[derive(Debug, Clone)]
pub enum Event {
    /// Analog stick input: x, y, side
    Stick { x: i32, y: i32, side: Side },
    /// Trigger input (ABS_Z=LT, ABS_RZ=RT)
    Trigger { value: i32, side: Side },
    /// Button input (EV_KEY event)
    Button { code: u16, pressed: bool },
}

impl Event {
    pub fn code(&self) -> u16 {
        match self {
            Event::Button { code, .. } => *code,
            _ => 0,
        }
    }
    pub fn x(&self) -> i32 {
        match self {
            Event::Stick { x, .. } => *x,
            _ => 0,
        }
    }
    pub fn y(&self) -> i32 {
        match self {
            Event::Stick { y, .. } => *y,
            _ => 0,
        }
    }
    pub fn value(&self) -> i32 {
        match self {
            Event::Trigger { value, .. } => *value,
            _ => 0,
        }
    }
    pub fn pressed(&self) -> bool {
        match self {
            Event::Button { pressed, .. } => *pressed,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn as_str(&self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }
}

/// Emitted output event from plugins.
#[derive(Debug, Clone)]
pub struct EmitEvent {
    pub ev_type: u16, // EV_ABS or EV_KEY
    pub code: u16,
    pub value: i32,
    /// If set and value==1 (KEY down), schedule a release after this many ms.
    pub hold_ms: Option<u64>,
}

/// Pipeline context including settings and emit buffer.
///
/// Emits that target the axes/button of the event currently in flight are
/// folded back into that event so later plugins see them; everything else goes
/// straight to the virtual device.
pub struct Ctx {
    pub settings: std::collections::HashMap<String, String>,
    pub emits: Vec<EmitEvent>,
    pub drop_original: bool,
    pub(crate) folded: bool,
}

/// Trait for a processing step.
pub trait Processor: Send + Sync {
    #[allow(dead_code)]
    fn id(&self) -> &str;
    fn process(&self, event: &mut Event, ctx: &mut Ctx);
}

/// Ordered pipeline.
pub struct Pipeline {
    steps: Vec<Box<dyn Processor>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Pipeline { steps: Vec::new() }
    }

    pub fn add(&mut self, p: Box<dyn Processor>) {
        self.steps.push(p);
    }
    #[cfg(test)]
    pub(crate) fn sort_steps_by_id(&mut self) {
        self.steps.sort_by_key(|step| step.id().to_string());
    }

    #[cfg(test)]
    pub(crate) fn reverse_steps(&mut self) {
        self.steps.reverse();
    }

    /// Reorder steps to follow `order` (ids first, in list order; unlisted
    /// ids keep their relative order, appended after).
    pub fn reorder(&mut self, order: &[String]) {
        self.steps
            .sort_by_key(|step| match order.iter().position(|id| id == step.id()) {
                Some(index) => (index, String::new()),
                None => (order.len(), step.id().to_string()),
            });
    }

    pub fn run(
        &self,
        event: &mut Event,
        settings: &std::collections::HashMap<String, String>,
    ) -> (Vec<EmitEvent>, bool) {
        let mut ctx = Ctx {
            settings: settings.clone(),
            emits: Vec::new(),
            drop_original: false,
            folded: false,
        };
        for step in &self.steps {
            step.process(event, &mut ctx);
            fold_emits(event, &mut ctx);
        }
        // A plugin that both dropped the original and emitted a replacement
        // (deadzone, button remap) has replaced the event content — keep it.
        if ctx.folded {
            ctx.drop_original = false;
        }
        (ctx.emits, ctx.drop_original)
    }

    #[allow(dead_code)]
    pub fn plugin_ids(&self) -> Vec<String> {
        self.steps.iter().map(|s| s.id().to_string()).collect()
    }
}

/// Move emits that belong to the in-flight event into that event.
fn fold_emits(event: &mut Event, ctx: &mut Ctx) {
    if ctx.emits.is_empty() {
        return;
    }
    let mut passthrough = Vec::with_capacity(ctx.emits.len());
    for emit in ctx.emits.drain(..) {
        // Emits with a hold duration are macros, not replacements.
        if emit.hold_ms.is_none() && try_fold(&emit, event) {
            ctx.folded = true;
        } else {
            passthrough.push(emit);
        }
    }
    ctx.emits = passthrough;
}

fn try_fold(emit: &EmitEvent, event: &mut Event) -> bool {
    match event {
        Event::Stick { x, y, side } => {
            let axis = match (side, emit.code) {
                (Side::Left, c) if c as u32 == ABS_X => Some(x),
                (Side::Left, c) if c as u32 == ABS_Y => Some(y),
                (Side::Right, c) if c as u32 == ABS_RX => Some(x),
                (Side::Right, c) if c as u32 == ABS_RY => Some(y),
                _ => None,
            };
            match axis {
                Some(target) => {
                    *target = emit.value;
                    true
                }
                None => false,
            }
        }
        Event::Trigger { value, side } => {
            let hit = (*side == Side::Left && emit.code as u32 == ABS_Z)
                || (*side == Side::Right && emit.code as u32 == ABS_RZ);
            if hit {
                *value = emit.value;
            }
            hit
        }
        Event::Button { code, pressed } => {
            if emit.ev_type == EV_KEY as u16 {
                *code = emit.code;
                *pressed = emit.value != 0;
                true
            } else {
                false
            }
        }
    }
}
