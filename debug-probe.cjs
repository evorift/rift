#!/usr/bin/env node
/**
 * evorift — Discord Endpoint Probe
 * TCP / UDP-DNS / HTTPS / QUIC (RFC 9001 Initial) testleri — 5 sn'de bir.
 * Çalıştır: node debug-probe.js
 * Durdur:   Ctrl+C
 */
'use strict';

const net    = require('net');
const dgram  = require('dgram');
const https  = require('https');
const dns    = require('dns');
const crypto = require('crypto');

// ─── Hedefler ────────────────────────────────────────────────────────────────

const TCP_TARGETS = [
  'discord.com',
  'gateway.discord.gg',
  'cdn.discordapp.com',
  'discordapp.com',
  'discord.media',
];

const HTTPS_TARGETS = [
  { host: 'discord.com',  path: '/api/v10' },
  { host: 'roblox.com',   path: '/' },
  { host: 'youtube.com',  path: '/' },
];

const QUIC_TARGETS = [
  'discord.com',
  'discordapp.com',
];

// ─── Renkler ─────────────────────────────────────────────────────────────────

const G = '\x1b[32m', R = '\x1b[31m', Y = '\x1b[33m',
      C = '\x1b[36m', D = '\x1b[2m',  B = '\x1b[1m', X = '\x1b[0m';

const OK   = ms  => `${G}✓${X} ${D}${String(ms).padStart(4)}ms${X}`;
const FAIL = ms  => `${R}✗${X} ${D}${ms >= 3000 ? 'timeout' : `${ms}ms`}${X}`;
const WARN = msg => `${Y}?${X} ${D}${msg}${X}`;
const pad  = (s, n) => String(s).padEnd(n);

// ─── TCP probe ────────────────────────────────────────────────────────────────

function probeTcp(host, port = 443) {
  return new Promise(resolve => {
    const t = Date.now();
    const s = net.createConnection({ host, port, timeout: 3000 });
    s.once('connect', () => { s.destroy(); resolve({ ok: true,  ms: Date.now() - t }); });
    s.once('timeout', () => { s.destroy(); resolve({ ok: false, ms: 3000 }); });
    s.once('error',   () =>               resolve({ ok: false, ms: Date.now() - t }));
  });
}

// ─── HTTPS probe ──────────────────────────────────────────────────────────────

function probeHttps(host, path) {
  return new Promise(resolve => {
    const t = Date.now();
    const req = https.request({ host, path, method: 'HEAD', timeout: 5000 }, res => {
      res.resume();
      resolve({ ok: true, ms: Date.now() - t, status: res.statusCode });
    });
    req.on('timeout', () => { req.destroy(); resolve({ ok: false, ms: 5000,         status: 0 }); });
    req.on('error',   () =>                  resolve({ ok: false, ms: Date.now()-t,  status: 0 }));
    req.end();
  });
}

// ─── UDP DNS probe ────────────────────────────────────────────────────────────

function probeUdpDns() {
  return new Promise(resolve => {
    const t  = Date.now();
    const cl = dgram.createSocket('udp4');
    let done = false;
    const finish = ok => {
      if (done) return; done = true;
      clearTimeout(timer);
      try { cl.close(); } catch (_) {}
      resolve({ ok, ms: Date.now() - t });
    };
    const timer = setTimeout(() => finish(false), 3000);
    // DNS query: discord.com A
    const q = Buffer.from([
      0x12,0x34, 0x01,0x00, 0x00,0x01, 0x00,0x00, 0x00,0x00, 0x00,0x00,
      0x07,...Buffer.from('discord'), 0x03,...Buffer.from('com'), 0x00,
      0x00,0x01, 0x00,0x01,
    ]);
    cl.send(q, 53, '1.1.1.1', err => {
      if (err) return finish(false);
      cl.once('message', () => finish(true));
      cl.once('error',   () => finish(false));
    });
  });
}

// ─── QUIC v1 Initial probe (RFC 9001) ────────────────────────────────────────
// Gerçek, şifreli QUIC Client Initial paketi üretip UDP/443'e gönderir.
// Sunucu cevap verirse (Server Initial / Retry / Version Negotiation) → ✓.

const QUIC_SALT = Buffer.from('38762cf7f55934b34d179ae6a4c80cadccbb7f0a','hex');

function hkdfExtract(salt, ikm) {
  return crypto.createHmac('sha256', salt).update(ikm).digest();
}

function hkdfExpand(prk, info, len) {
  const out = Buffer.alloc(len);
  let prev = Buffer.alloc(0), off = 0;
  for (let i = 1; off < len; i++) {
    prev = crypto.createHmac('sha256', prk)
      .update(Buffer.concat([prev, info, Buffer.from([i])]))
      .digest();
    const n = Math.min(32, len - off);
    prev.copy(out, off, 0, n);
    off += n;
  }
  return out;
}

function hkdfExpandLabel(secret, label, len) {
  const lbl  = Buffer.from(`tls13 ${label}`);
  const info = Buffer.concat([
    Buffer.from([0, len]), Buffer.from([lbl.length]), lbl, Buffer.from([0]),
  ]);
  return hkdfExpand(secret, info, len);
}

function quicKeys(dcid) {
  const is = hkdfExtract(QUIC_SALT, dcid);
  const cs = hkdfExpandLabel(is, 'client in', 32);
  return {
    key: hkdfExpandLabel(cs, 'quic key', 16),
    iv:  hkdfExpandLabel(cs, 'quic iv',  12),
    hp:  hkdfExpandLabel(cs, 'quic hp',  16),
  };
}

function aeadEncrypt(key, iv, plaintext, aad) {
  const c = crypto.createCipheriv('aes-128-gcm', key, iv);
  c.setAAD(aad);
  const ct  = Buffer.concat([c.update(plaintext), c.final()]);
  return Buffer.concat([ct, c.getAuthTag()]);
}

function hpMask(hp, sample) {
  const c = crypto.createCipheriv('aes-128-ecb', hp, null);
  c.setAutoPadding(false);
  return c.update(sample).slice(0, 5);
}

function varint(v) {
  if (v < 64)    return Buffer.from([v]);
  if (v < 16384) return Buffer.from([0x40 | (v >> 8), v & 0xff]);
  return Buffer.from([0x80|(v>>24), (v>>16)&0xff, (v>>8)&0xff, v&0xff]);
}

function buildClientHello(sni) {
  const sniB = Buffer.from(sni);
  const sniExt = Buffer.concat([
    Buffer.from([0,0]),                                // ext type: server_name
    Buffer.from([0, 5 + sniB.length]),                 // ext len
    Buffer.from([0, 3 + sniB.length]),                 // list len
    Buffer.from([0]),                                  // name type: host_name
    Buffer.from([sniB.length >> 8, sniB.length & 0xff]),
    sniB,
  ]);
  // supported_versions: TLS 1.3
  const svExt = Buffer.from([0,0x2b, 0,3, 2, 0x03,0x04]);
  // supported_groups: x25519
  const sgExt = Buffer.from([0,0x0a, 0,4, 0,2, 0,0x1d]);
  // key_share: x25519 placeholder
  const ks = crypto.randomBytes(32);
  const ksExt = Buffer.concat([
    Buffer.from([0,0x33, 0, 38, 0, 36, 0,0x1d, 0,32]), ks,
  ]);
  const exts = Buffer.concat([sniExt, svExt, sgExt, ksExt]);
  const body = Buffer.concat([
    Buffer.from([0x03,0x03]), crypto.randomBytes(32),
    Buffer.from([0]),
    Buffer.from([0,2, 0x13,0x01]),   // TLS_AES_128_GCM_SHA256
    Buffer.from([1,0]),
    Buffer.from([exts.length >> 8, exts.length & 0xff]), exts,
  ]);
  return Buffer.concat([
    Buffer.from([1, 0, body.length >> 8, body.length & 0xff]), body,
  ]);
}

function buildQuicInitial(sni) {
  const dcid = crypto.randomBytes(8);
  const keys = quicKeys(dcid);
  const ch   = buildClientHello(sni);

  // CRYPTO frame
  const cryptoFrame = Buffer.concat([
    Buffer.from([0x06]), varint(0), varint(ch.length), ch,
  ]);

  // payload = CRYPTO + PADDING → hdr+pn+payload+tag >= 1200
  // header overhead ≈ 1+4+1+8+1+1+2+1 = 19 bytes; so payload >= 1200-19-16 = 1165
  const padLen = Math.max(0, 1165 - cryptoFrame.length);
  const plaintext = Buffer.concat([cryptoFrame, Buffer.alloc(padLen, 0)]);

  const pnLen      = 1;
  const payloadLen = pnLen + plaintext.length + 16; // PN + payload + AEAD tag

  // QUIC Long Header (Initial, PN length = 1 byte → first byte low 2 bits = 0b00)
  const hdrBody = Buffer.concat([
    Buffer.from([0xC0]),          // Long header | Fixed bit | Initial (type=0) | PN_len=1
    Buffer.from([0,0,0,1]),       // QUIC v1
    Buffer.from([dcid.length]), dcid,
    Buffer.from([0]),             // SCID len
    Buffer.from([0]),             // Token len
    varint(payloadLen),
    Buffer.from([0]),             // Packet Number = 0
  ]);

  // AEAD nonce = IV XOR 0
  const nonce = Buffer.from(keys.iv);

  const ct  = aeadEncrypt(keys.key, nonce, plaintext, hdrBody);
  const pkt = Buffer.concat([hdrBody, ct]);

  // Header protection
  const pnOffset  = hdrBody.length - 1;
  const sampleOff = pnOffset + 4;
  const mask      = hpMask(keys.hp, pkt.slice(sampleOff, sampleOff + 16));
  const result    = Buffer.from(pkt);
  result[0]       ^= mask[0] & 0x0f;
  result[pnOffset]^= mask[1];

  return result;
}

function probeQuic(host) {
  return new Promise(resolve => {
    const t   = Date.now();
    let done  = false;
    const finish = ok => {
      if (done) return; done = true;
      clearTimeout(timer);
      resolve({ ok, ms: Date.now() - t });
    };
    const timer = setTimeout(() => finish(false), 3000);

    dns.resolve4(host, (err, addrs) => {
      if (err || !addrs || !addrs.length) return finish(false);
      const addr = addrs[0];
      let pkt;
      try   { pkt = buildQuicInitial(host); }
      catch (_) { return finish(false); }

      const cl = dgram.createSocket('udp4');
      cl.once('error', () => finish(false));
      cl.send(pkt, 443, addr, sendErr => {
        if (sendErr) { try { cl.close(); } catch (_) {} return finish(false); }
        cl.once('message', () => { try { cl.close(); } catch (_) {} finish(true); });
      });
    });
  });
}

// ─── Ana döngü ────────────────────────────────────────────────────────────────

let count = 0;

async function run() {
  count++;
  const ts = new Date().toLocaleTimeString('tr-TR', { hour12: false });
  console.log(`\n${B}${C}── #${count}  ${ts} ─────────────────────────────────────────${X}`);

  const [tcpRes, udpRes, httpsRes, quicRes] = await Promise.all([
    Promise.all(TCP_TARGETS.map(h   => probeTcp(h).then(r       => ({ host: h,       ...r })))),
    probeUdpDns(),
    Promise.all(HTTPS_TARGETS.map(t => probeHttps(t.host, t.path).then(r => ({ ...t, ...r })))),
    Promise.all(QUIC_TARGETS.map(h  => probeQuic(h).then(r      => ({ host: h,       ...r })))),
  ]);

  // TCP
  process.stdout.write(`\n  ${D}TCP/443${X}\n`);
  for (const r of tcpRes) {
    const st = r.ok ? OK(r.ms) : FAIL(r.ms);
    console.log(`    ${pad(r.host, 32)} ${st}`);
  }

  // UDP DNS
  process.stdout.write(`\n  ${D}UDP/53  (DNS — proves UDP works)${X}\n`);
  const d = udpRes;
  console.log(`    ${pad('1.1.1.1:53', 32)} ${d.ok ? OK(d.ms) : FAIL(d.ms)}`);

  // HTTPS
  process.stdout.write(`\n  ${D}HTTPS (full TLS stack)${X}\n`);
  for (const r of httpsRes) {
    const lbl = r.ok ? `${r.host}  ${D}(HTTP ${r.status})${X}` : r.host;
    console.log(`    ${pad(r.host, 32)} ${r.ok ? OK(r.ms) : FAIL(r.ms)}  ${r.ok ? `${D}HTTP ${r.status}${X}` : ''}`);
  }

  // QUIC
  process.stdout.write(`\n  ${D}QUIC/UDP-443  (RFC 9001 Initial — sunucu cevabı = geçti)${X}\n`);
  for (const r of quicRes) {
    const note = r.ok ? '' : r.ms < 200
      ? `  ${Y}⚡ hızlı red — ISP/DPI ICMP unreachable gönderiyor${X}`
      : `  ${D}sessiz drop — DPI paketi yuttu${X}`;
    console.log(`    ${pad(r.host, 32)} ${r.ok ? OK(r.ms) : FAIL(r.ms)}${note}`);
  }

  console.log('');
}

console.log(`${B}evorift Discord Probe  —  5 sn aralık  (Ctrl+C = dur)${X}`);
run();
setInterval(run, 5000);
