// Drive the normal-buffer scrollbar of the live Console on a synthetic PTY.
import assert from "node:assert/strict";

export async function checkConsoleScroll({ step, web, probe, rpc, report }) {
  const id = "claudeBridgeConsoleView";
  await step("create-synthetic-claude-pty", () =>
    web("window.bridgeCmd.raw('newAgentSession','claude'); return true"));
  await step("claude-pty-prompt-ready", async () => {
    const result = await web(`const s=window.bridgeCmd.state();
      const tab=(await window.bridgeCmd.raw('listTabs')).find(t=>t.id===s.activeTabId);
      return s.promptReady && tab?.engine==='claude' && tab.status==='connected'`);
    assert.equal(result.value, true);
  }, true);
  await step("open-native-console-panel", () => rpc("kamin:command:execute", [id + ".focus"]));
  const terminal = body => probe({ cmd: "wveval", id, body, timeoutMs: 6000 });
  const state = async () => (await terminal(`const c=Array.from(document.querySelectorAll('.bridge-console')).find(el=>el.getClientRects().length);
    const track=c?.querySelector('.xterm-scrollable-element>.scrollbar.vertical');
    const slider=track?.querySelector('.slider');
    if (!track || !slider || c.dataset.terminalScreen!=='normal') return null;
    const a=track.getBoundingClientRect(), b=slider.getBoundingClientRect(), s=c.getBoundingClientRect();
    const travel=a.height-b.height;
    return { history:c.dataset.terminalHistory==='true', travel, pos:travel>0?(b.top-a.top)/travel:0,
      width:innerWidth, height:innerHeight, overflow:document.documentElement.scrollWidth-innerWidth,
      x:s.left+s.width/2, y:s.top+s.height/2, thumbX:b.left+b.width/2, thumbY:b.top+b.height/2,
      top:a.top+b.height/2, bottom:a.bottom-b.height/2,
      thumbHit:track.contains(document.elementFromPoint(b.left+b.width/2,b.top+b.height/2)) }`)).value;
  async function mouse(action, x, y, extras = {}) {
    const result = await probe({ cmd: "webmouse", id, action, x, y, ...extras });
    assert.equal(result.delivery, "cef-host");
  }
  await step("live-normal-console", async () => assert.ok(await state()), true);
  let tab;
  await step("observe-numbered-output", async () => {
    tab = (await web("return window.bridgeCmd.state().activeTabId")).value;
    const active = JSON.stringify(tab);
    const { value } = await terminal(`window.smokeScrollOutput={events:0,newlines:0,marker:false};
      window.kaminBridge.onOutput((id,data)=>{
        if (id!==${active}) return;
        const out=window.smokeScrollOutput;
        out.events++;
        out.newlines+=data.split(String.fromCharCode(10)).length-1;
        if (data.includes('native-scroll')) out.marker=true;
      }); return true`);
    assert.equal(value, true);
  });
  // A single Windows ConPTY burst can repaint only the last screen. The
  // existing streamed fixture emits a numbered line at each time interval.
  await step("write-numbered-normal-lines", async () => {
    const { value } = await web("return window.bridgeCmd.send('/fake-stream 80 80 native-scroll')");
    assert.equal(value, tab);
  });
  let bottomState;
  await step("history-and-visible-scrollbar", async () => {
    bottomState = await state();
    report.scrollGeometry = { start: bottomState };
    report.consoleOutput = (await terminal("return window.smokeScrollOutput")).value;
    assert.ok(report.consoleOutput.marker && report.consoleOutput.newlines >= 25);
    assert.ok(bottomState?.history && bottomState.travel>8 && bottomState.pos>0.9);
    assert.ok(bottomState.overflow<=1 && bottomState.width>16 && bottomState.height>16);
  }, true);
  await step("wheel-towards-older-lines", () => mouse("wheel", bottomState.x, bottomState.y, { dy: 720 }));
  let priorState;
  await step("wheel-moves-towards-history", async () => {
    priorState = await state();
    report.scrollGeometry.afterWheel = priorState?.pos;
    assert.ok(priorState?.pos < bottomState.pos-0.01);
  }, true);
  async function dragTo(edge, start) {
    const x=start.thumbX, y=start.thumbY, end=start[edge];
    await mouse("move", x, y);
    await mouse("down", x, y);
    await mouse("move", x, (y+end)/2, { held:true });
    await mouse("move", x, end, { held:true });
    await mouse("up", x, end);
  }
  await step("drag-terminal-thumb-to-top", () => dragTo("top", priorState));
  let atTop;
  await step("thumb-reaches-top", async () => {
    atTop = await state();
    report.scrollGeometry.afterUpDrag = atTop?.pos;
    assert.ok(atTop?.pos <= 0.02);
  }, true);
  await step("drag-terminal-thumb-to-bottom", () => dragTo("bottom", atTop));
  await step("thumb-reaches-bottom", async () => {
    const actual = await state();
    report.scrollGeometry.afterDownDrag = actual?.pos;
    assert.ok(actual?.history && actual.pos >= 0.98);
    report.console = { screen: "normal", wheelMoved: true, thumbReachedTop: true, thumbReachedBottom: true,
      outsideHorizontalOverflow: actual.overflow };
  }, true);
}
