/**
 * Example: Connect to RDP, open PowerShell, and type "echo hello world"
 *
 * Usage:
 *   npx tsx examples/hello-world.ts --host <ip> --username <user> --password <pass> [--drive /path:Name]
 *
 * Or with environment variables:
 *   AGENT_RDP_HOST=192.168.1.100 AGENT_RDP_USERNAME=Admin AGENT_RDP_PASSWORD=secret npx tsx examples/hello-world.ts
 */

import { RdpSession, DriveMapping } from '../src/index.js';

// Parse arguments
function getArg(name: string, envVar?: string): string | undefined {
  const args = process.argv.slice(2);
  const idx = args.indexOf(`--${name}`);
  if (idx !== -1 && args[idx + 1]) {
    return args[idx + 1];
  }
  return envVar ? process.env[envVar] : undefined;
}

// Parse drive mappings (--drive /path:Name)
function getDrives(): DriveMapping[] {
  const drives: DriveMapping[] = [];
  const args = process.argv.slice(2);

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--drive' && args[i + 1]) {
      const [drivePath, name] = args[i + 1].split(':');
      if (drivePath && name) {
        drives.push({ path: drivePath, name });
      }
    }
  }

  return drives;
}

const host = getArg('host', 'AGENT_RDP_HOST');
const username = getArg('username', 'AGENT_RDP_USERNAME');
const password = getArg('password', 'AGENT_RDP_PASSWORD');
const driveMappings = getDrives();

if (!host || !username || !password) {
  console.error('Usage: npx tsx examples/hello-world.ts --host <ip> --username <user> --password <pass> [--drive /path:Name]');
  console.error('Or set AGENT_RDP_HOST, AGENT_RDP_USERNAME, AGENT_RDP_PASSWORD environment variables');
  process.exit(1);
}

async function main() {
  const rdp = new RdpSession({ session: 'example' });

  console.log(`Connecting to ${host}...`);

  await rdp.connect({
    host,
    username,
    password,
    width: 1280,
    height: 800,
    drives: driveMappings,
  });

  console.log('Connected! Waiting for desktop to load...');
  await sleep(3000);

  // Take a screenshot to see the desktop
  const shot1 = await rdp.screenshot({ format: 'png' });
  console.log(`Desktop screenshot: ${shot1.width}x${shot1.height}`);

  // Open PowerShell via Win+R (Run dialog)
  console.log('Opening PowerShell via Win+R...');

  // Press Win+R to open Run dialog
  await rdp.keyboard.press('win+r');
  await sleep(1000);

  // Type "powershell"
  await rdp.keyboard.type('powershell');
  await sleep(500);

  // Press Enter to open it
  await rdp.keyboard.key('enter');
  await sleep(2000);

  // Take a screenshot to see PowerShell
  const shot2 = await rdp.screenshot({ format: 'png' });
  console.log(`PowerShell screenshot: ${shot2.width}x${shot2.height}`);

  // Type our command
  console.log('Typing echo hello world...');
  await rdp.keyboard.type('echo hello world');
  await sleep(500);

  // Press Enter to execute
  await rdp.keyboard.key('enter');
  await sleep(1000);

  // Take final screenshot
  const shot3 = await rdp.screenshot({ format: 'png' });
  console.log(`Final screenshot: ${shot3.width}x${shot3.height}`);

  // Save screenshots to disk
  const fs = await import('node:fs');
  fs.writeFileSync('screenshot-1-desktop.png', Buffer.from(shot1.base64, 'base64'));
  fs.writeFileSync('screenshot-2-powershell.png', Buffer.from(shot2.base64, 'base64'));
  fs.writeFileSync('screenshot-3-result.png', Buffer.from(shot3.base64, 'base64'));
  console.log('Screenshots saved to screenshot-*.png');

  // List the mapped drives
  const mappedDrives = await rdp.drives.list();
  console.log('Mapped drives:', mappedDrives);

  // Disconnect
  console.log('Disconnecting...');
  await rdp.disconnect();

  console.log('Done!');
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((err) => {
  console.error('Error:', err);
  process.exit(1);
});
