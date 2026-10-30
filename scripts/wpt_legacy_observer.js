// РљРѕРЅС‚РµРєСЃС‚ readback СЃС‚Р°СЂРѕРіРѕ baseline; РёРЅСЃС‚СЂСѓРјРµРЅС‚РёСЂСѓРµС‚СЃСЏ С‚РѕР»СЊРєРѕ СЃРѕР·РґР°РЅРЅС‹Р№ runner child.
const hooks = new Set(), devices = new Map(), mapped = new Map(), copies = new Map();
let nested = false, serial = 0, factoryDebug = false;
function method(p, i, ret, args) {
    return new NativeFunction(p.readPointer().add(i * Process.pointerSize).readPointer(), ret, ['pointer', ...args]);
}
function release(p) { method(p, 2, 'uint', [])(p); }
function hook(p, i, callbacks) {
    const address = p.readPointer().add(i * Process.pointerSize).readPointer();
    if (hooks.has(address.toString())) return;
    hooks.add(address.toString());
    Interceptor.attach(address, callbacks);
}
function guid(bytes) { const p = Memory.alloc(16); p.writeByteArray(bytes); return p; }
const dxgiDevice = guid([250,119,236,84,119,19,230,68,140,50,136,253,95,68,200,76]);
const adapter1 = guid([97,143,3,41,57,56,38,70,145,253,8,104,121,1,26,5]);
function query(p, iid) {
    const out = Memory.alloc(Process.pointerSize);
    if (method(p, 0, 'int', ['pointer','pointer'])(p, iid, out) < 0) throw Error('QueryInterface failed');
    return out.readPointer();
}
function description(adapter) {
    const b = Memory.alloc(320), version = Memory.alloc(8);
    if (method(adapter, 10, 'int', ['pointer'])(adapter,b) < 0) throw Error('GetDesc1 failed');
    if (method(adapter, 9, 'int', ['pointer','pointer'])(adapter,dxgiDevice,version) < 0) throw Error('UMD version unknown');
    return {luid_high_part:b.add(300).readS32(), luid_low_part:b.add(296).readU32(),
        device_name:b.readUtf16String(),vendor_id:b.add(256).readU32(),device_id:b.add(260).readU32(),
        subsystem_id:b.add(264).readU32(),revision:b.add(268).readU32(),adapter_flags:b.add(304).readU32(),
        driver_version:[6,4,2,0].map(o=>version.add(o).readU16()).join('.')};
}
function identity(device) {
    const dxgi = query(device, dxgiDevice), out = Memory.alloc(Process.pointerSize);
    if (method(dxgi, 7, 'int', ['pointer'])(dxgi,out) < 0) throw Error('GetAdapter failed');
    const base = out.readPointer(), adapter = query(base, adapter1);
    const result = description(adapter);
    release(adapter); release(base); release(dxgi);
    return {...result,feature_level:method(device,37,'uint',[])(device),
        creation_flags:method(device,38,'uint',[])(device),factory_debug:factoryDebug,
        backend:'d3d11',queried_from_device:true};
}
function executable(bytes) {
    const b = new Uint8Array(bytes), view = new DataView(bytes);
    if (String.fromCharCode(...b.slice(0,4)) !== 'DXBC') throw Error('Unknown shader container');
    const chunks = [];
    for (let i=0;i<view.getUint32(28,true);i++) {
        const offset=view.getUint32(32+i*4,true), tag=String.fromCharCode(...b.slice(offset,offset+4));
        if (tag==='SHDR'||tag==='SHEX') chunks.push(...b.slice(offset,offset+8+view.getUint32(offset+4,true)));
    }
    if (!chunks.length) throw Error('No shader executable');
    return Checksum.compute('sha256',new Uint8Array(chunks).buffer);
}
function contextHooks(context) {
    hook(context,47,{onEnter(a){ copies.set(a[1].toString(),a[2]); }});
    hook(context,14,{
        onEnter(a){this.ctx=a[0];this.resource=a[1];this.mode=a[3].toUInt32();this.out=a[5];},
        onLeave(result){
            if (result.toInt32()<0) return;
            const dim=Memory.alloc(4); method(this.resource,7,'void',['pointer'])(this.resource,dim);
            const desc=Memory.alloc(44); method(this.resource,10,'void',['pointer'])(this.resource,desc);
            const data=this.out.readPointer();
            if (dim.readU32()===1 && desc.readU32()===48 && (desc.add(8).readU32()&4)) {
                mapped.set(this.resource.toString(),{ctx:this.ctx,data}); return;
            }
            if (dim.readU32()!==3 || this.mode!==1 || desc.add(28).readU32()!==3) return;
            const source=copies.get(this.resource.toString());
            if (!source) throw Error('Readback has no source texture');
            const target=Memory.alloc(44);method(source,10,'void',['pointer'])(source,target);
            const width=desc.readU32(),height=desc.add(4).readU32(),pitch=this.out.add(8).readU32();
            const deviceOut=Memory.alloc(Process.pointerSize);method(this.ctx,3,'void',['pointer'])(this.ctx,deviceOut);
            const device=deviceOut.readPointer(), state=devices.get(device.toString());
            if (!state || !state.gamma) throw Error('Unattested device or gamma state');
            const bytes=Memory.alloc(width*height*4);
            for(let y=0;y<height;y++) Memory.copy(bytes.add(y*width*4),data.add(y*pitch),width*4);
            const actual={...identity(device),generation:state.generation};release(device);
            send({event:'capture',width,height,bgra_sha256:Checksum.compute('sha256',bytes.readByteArray(width*height*4)),
                context:{device:actual,target_format:target.add(16).readS32(),sample_count:target.add(20).readU32(),
                    sample_quality:target.add(24).readU32(),gamma_ratios:state.gamma.ratios,
                    grayscale_enhanced_contrast:state.gamma.contrast,shader_executables:[...state.shaders].sort()}});
        }
    });
    hook(context,15,{onEnter(a){
        const row=mapped.get(a[1].toString()); if(!row) return;
        mapped.delete(a[1].toString());
        const out=Memory.alloc(Process.pointerSize);method(a[0],3,'void',['pointer'])(a[0],out);
        const device=out.readPointer(),state=devices.get(device.toString());
        if(state) state.gamma={ratios:[0,4,8,12].map(o=>row.data.add(o).readFloat()),contrast:row.data.add(24).readFloat()};
        release(device);
    }});
}
Process.attachModuleObserver({onAdded(module){
    // РЎС‚Р°СЂС‹Р№ baseline СЃРѕР·РґР°С‘С‚ СЃРІРѕС‘ HWND РґРѕ РЅР°СЃС‚СЂРѕР№РєРё focus: СѓРґРµСЂР¶РёРІР°РµРј РµРіРѕ Р±РµР· Р°РєС‚РёРІР°С†РёРё.
    if(module.name.toLowerCase()==='user32.dll') {
        Interceptor.attach(module.getExportByName('CreateWindowExW'),{onEnter(a){
            if(!(a[3].toUInt32()&0x40000000)) {
                a[0]=ptr(a[0].toUInt32()|0x08000000);a[4]=ptr(-8000);a[5]=ptr(-8000);
            }
        }});
        Interceptor.attach(module.getExportByName('ShowWindow'),{onEnter(a){if(a[1].toInt32())a[1]=ptr(4);}});
        Interceptor.attach(module.getExportByName('SetWindowPos'),{onEnter(a){a[6]=ptr(a[6].toUInt32()|0x214);}});
        for(const name of ['SetForegroundWindow','SetActiveWindow','SetFocus'])
            Interceptor.replace(module.getExportByName(name),new NativeCallback(()=>ptr(0),'pointer',['pointer']));
    }
    if(module.name.toLowerCase()==='dxgi.dll') Interceptor.attach(module.getExportByName('CreateDXGIFactory2'),{
        onEnter(a){this.flags=a[0].toUInt32();this.out=a[2];},onLeave(result){
            if(result.toInt32()<0) return; factoryDebug=Boolean(this.flags&1);
            const factory=this.out.readPointer();
            hook(factory,7,{onEnter(a){this.factory=a[0];this.index=a[1].toUInt32();this.out=a[2];this.nested=nested;},onLeave(result){
                if(result.toInt32()<0||this.nested||this.index!==0) return;
                nested=true;const matches=[];
                try {
                    for(let i=0;;i++) {
                        const out=Memory.alloc(Process.pointerSize), hr=method(this.factory,7,'int',['uint','pointer'])(this.factory,i,out);
                        if(hr===-2005270526) break;
                        if(hr<0) throw Error('EnumAdapters failed');
                        const base=out.readPointer(),adapter=query(base,adapter1),info=description(adapter);release(adapter);
                        if(info.luid_high_part===requested.high_part && info.luid_low_part===requested.low_part && !(info.adapter_flags&2)) matches.push(base);
                        else release(base);
                    }
                    if(matches.length!==1) throw Error('Strict legacy ByLuid did not match exactly one hardware adapter');
                    release(this.out.readPointer());this.out.writePointer(matches[0]);
                    send({event:'strict-selector',resolved:requested});
                } finally { nested=false; }
            }});
        }
    });
    if(module.name.toLowerCase()!=='d3d11.dll') return;
    Interceptor.attach(module.getExportByName('D3D11CreateDevice'),{
        onEnter(a){this.dev=a[7];this.ctx=a[9];},onLeave(result){
            if(result.toInt32()<0||this.dev.isNull()) return;
            const device=this.dev.readPointer(),state={generation:++serial,shaders:new Set(),gamma:null};
            devices.set(device.toString(),state);
            send({event:'device',context:{...identity(device),generation:state.generation},
                modules:Process.enumerateModules().map(m=>({name:m.name.toLowerCase(),path:m.path}))});
            for(const index of [12,15]) hook(device,index,{
                onEnter(a){this.state=devices.get(a[0].toString());if(this.state)this.hash=executable(a[1].readByteArray(a[2].toUInt32()));},
                onLeave(r){if(r.toInt32()>=0&&this.state)this.state.shaders.add(this.hash);}
            });
            if(!this.ctx.isNull()) contextHooks(this.ctx.readPointer());
        }
    });
}});
