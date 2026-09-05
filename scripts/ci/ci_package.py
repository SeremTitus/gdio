import re, sys, shutil
from pathlib import Path

def snake_case(s):
    raw = re.sub(r'[^a-z0-9]', '_', s.lower())
    return re.sub(r'_+', '_', raw).strip('_')

def parse_presets(cfg_path):
    if not cfg_path.exists():
        return []
    content = cfg_path.read_text(encoding='utf-8')
    presets, cur = [], {}
    for line in content.splitlines():
        line = line.strip()
        if line.startswith('[preset.'):
            if 'name' in cur: presets.append(cur)
            cur = {}
        elif line.startswith('['):
            if 'name' in cur: presets.append(cur)
            cur = {}
        else:
            for key in ('name', 'platform', 'export_path'):
                m = re.match(rf'^{key}="([^"]*)"', line)
                if m: cur[key] = m.group(1)
    if 'name' in cur: presets.append(cur)
    return presets

def compute_output_dir(preset, cwd):
    preset_snake = snake_case(preset.get('name', ''))
    export_path = preset.get('export_path', '')
    if export_path:
        out_file = cwd / export_path
        return out_file.parent if out_file.exists() else None
    out_dir = cwd / 'export' / preset_snake
    return out_dir if out_dir.exists() and any(out_dir.iterdir()) else None

if len(sys.argv) < 2:
    print('Usage: ci_package.py <output_folder>')
    sys.exit(1)

output_folder = Path.cwd() / sys.argv[1]
presets = parse_presets(Path.cwd() / 'export_presets.cfg')
if not presets:
    print('No export presets found.')
    sys.exit(0)

if output_folder.exists():
    shutil.rmtree(output_folder)
output_folder.mkdir(parents=True)

found = 0
for preset in presets:
    name = preset.get('name', 'unknown')
    ps = snake_case(name)
    out_dir = compute_output_dir(preset, Path.cwd())
    if out_dir is None:
        print(f'  {name} -> NOT FOUND')
        continue
    dest = output_folder / ps
    shutil.copytree(out_dir, dest)
    found += 1
    print(f'  {name} -> {out_dir}')

if found == 0:
    print('No exports produced.')
    shutil.rmtree(output_folder)
    sys.exit(1)

print(f'Created {sys.argv[1]}/ with {found} preset(s)')
