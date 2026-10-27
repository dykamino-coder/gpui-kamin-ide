//! KaminIDE patch (INC-2026-0001, диагностика): цена кадра по видам пачек.
//!
//! На машине без видеокарты каждый кадр окна растеризует WARP на CPU, и его
//! цена в секундные счётчики gpui не попадает: `Present` только ставит
//! команды в очередь. Здесь копятся число примитивов, их площадь в пикселях
//! (для программного растеризатора это прямая мера работы) и GPU-время каждой
//! пачки по timestamp-запросам D3D11. Результаты забираются с отставанием в
//! несколько кадров без ожидания — замер не тормозит кадр.
//!
//! Число и площадь копятся всегда: это несколько сложений на пачку, а для
//! программного растеризатора площадь и есть цена. Timestamp-запросы — только
//! при явном gpu_timing: это лишние объекты устройства и вызовы на
//! каждую пачку, нужные лишь при разборе.

use windows::Win32::Graphics::Direct3D11::*;

/// Сколько кадров ждут ответа видеокарты одновременно.
const RING: usize = 4;

struct Slot {
    disjoint: ID3D11Query,
    stamps: Vec<ID3D11Query>,
    kinds: Vec<usize>,
    used: usize,
    pending: bool,
}

/// Timestamp-запросы кадра: стартовая отметка, отметка после каждой пачки.
#[derive(Default)]
pub(crate) struct GpuTimer {
    pub(crate) instrumentation: gpui::Instrumentation,
    slots: Vec<Slot>,
    next: usize,
    current: Option<usize>,
    failed: bool,
    /// Устройство, на котором созданы запросы: после device lost рендерер
    /// создаёт новое, и запросы старого с ним использовать нельзя.
    device: usize,
}

impl GpuTimer {
    pub(crate) fn new(instrumentation: gpui::Instrumentation) -> Self {
        Self { instrumentation, ..Self::default() }
    }

    /// Начать кадр. Если слот ещё ждёт видеокарту, кадр идёт без GPU-времени.
    pub(crate) fn begin(&mut self, device: &ID3D11Device, ctx: &ID3D11DeviceContext) {
        self.instrumentation.batches.frame();
        self.current = None;
        if !self.instrumentation.config().gpu_timing {
            return;
        }
        let raw = windows::core::Interface::as_raw(device) as usize;
        if raw != self.device {
            *self = Self {
                device: raw,
                instrumentation: self.instrumentation.clone(),
                ..Self::default()
            };
        }
        if self.failed {
            return;
        }
        if self.slots.is_empty() {
            for _ in 0..RING {
                match create_slot(device) {
                    Some(slot) => self.slots.push(slot),
                    None => {
                        self.failed = true;
                        return;
                    }
                }
            }
        }
        for i in 0..RING {
            collect(&mut self.slots[i], ctx, &self.instrumentation);
        }
        let index = self.next;
        let slot = &mut self.slots[index];
        if slot.pending {
            return;
        }
        self.next = (index + 1) % RING;
        slot.used = 0;
        slot.kinds.clear();
        unsafe {
            ctx.Begin(&slot.disjoint);
            ctx.End(&slot.stamps[0]);
        }
        slot.used = 1;
        self.current = Some(index);
    }

    /// Отметка после пачки вида `kind`.
    pub(crate) fn mark(&mut self, device: &ID3D11Device, ctx: &ID3D11DeviceContext, kind: usize) {
        let Some(index) = self.current else { return };
        let slot = &mut self.slots[index];
        if slot.used == slot.stamps.len() {
            match create_timestamp(device) {
                Some(q) => slot.stamps.push(q),
                None => return,
            }
        }
        unsafe { ctx.End(&slot.stamps[slot.used]) };
        slot.used += 1;
        slot.kinds.push(kind);
    }

    /// Закончить кадр; ответ заберёт один из следующих `begin`.
    pub(crate) fn end(&mut self, ctx: &ID3D11DeviceContext) {
        let Some(index) = self.current.take() else { return };
        let slot = &mut self.slots[index];
        unsafe { ctx.End(&slot.disjoint) };
        slot.pending = true;
    }
}

fn create_query(device: &ID3D11Device, kind: D3D11_QUERY) -> Option<ID3D11Query> {
    let desc = D3D11_QUERY_DESC {
        Query: kind,
        MiscFlags: 0,
    };
    let mut query = None;
    unsafe { device.CreateQuery(&desc, Some(&mut query)) }.ok()?;
    query
}

fn create_timestamp(device: &ID3D11Device) -> Option<ID3D11Query> {
    create_query(device, D3D11_QUERY_TIMESTAMP)
}

fn create_slot(device: &ID3D11Device) -> Option<Slot> {
    let disjoint = create_query(device, D3D11_QUERY_TIMESTAMP_DISJOINT)?;
    let mut stamps = Vec::with_capacity(32);
    for _ in 0..32 {
        stamps.push(create_timestamp(device)?);
    }
    Some(Slot {
        disjoint,
        stamps,
        kinds: Vec::new(),
        used: 0,
        pending: false,
    })
}

/// Забрать готовый ответ слота без ожидания.
fn collect(slot: &mut Slot, ctx: &ID3D11DeviceContext, instrumentation: &gpui::Instrumentation) {
    if !slot.pending {
        return;
    }
    let mut disjoint = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT::default();
    if !ready(ctx, &slot.disjoint, &mut disjoint) {
        return;
    }
    slot.pending = false;
    if disjoint.Disjoint.as_bool() || disjoint.Frequency == 0 {
        return;
    }
    let mut stamps = Vec::with_capacity(slot.used);
    for q in &slot.stamps[..slot.used] {
        let mut value = 0u64;
        if !ready(ctx, q, &mut value) {
            return;
        }
        stamps.push(value);
    }
    let ns = |a: u64, b: u64| b.saturating_sub(a) as f64 * 1e9 / disjoint.Frequency as f64;
    let frame = match (stamps.first(), stamps.last()) {
        (Some(first), Some(last)) => ns(*first, *last),
        _ => 0.0,
    };
    let kinds: Vec<(usize, f64)> = slot
        .kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| (*kind, ns(stamps[i], stamps[i + 1])))
        .collect();
    instrumentation.batches.timed(frame, &kinds);
}

fn ready<T>(ctx: &ID3D11DeviceContext, query: &ID3D11Query, out: &mut T) -> bool {
    use windows::core::Interface;
    // Обёртка `GetData` сводит S_FALSE («ещё не готово») к Ok — различаем
    // готовность по сырому HRESULT из vtable.
    let hr = unsafe {
        (Interface::vtable(ctx).GetData)(
            Interface::as_raw(ctx),
            Interface::as_raw(query),
            out as *mut T as *mut _,
            std::mem::size_of::<T>() as u32,
            D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
        )
    };
    hr == windows::Win32::Foundation::S_OK
}
