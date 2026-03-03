#!/usr/bin/env python3
import json
from pathlib import Path

profiles_dir = Path.home() / '.llxprt' / 'profiles'
profiles_dir.mkdir(parents=True, exist_ok=True)

legacy_1 = {
    "version": 1,
    "provider": "openai",
    "model": "legacy/test-model-1",
    "modelParams": {},
    "ephemeralSettings": {
        "auth-keyfile": str(Path.home() / '.llxprt' / 'keys' / '.legacy_key_1'),
        "base-url": "https://api.synthetic.new/v1",
        "context-limit": 64000,
        "maxOutputTokens": 4096,
        "reasoning.enabled": True,
        "reasoning.includeInResponse": True,
    }
}
legacy_2 = {
    "version": 1,
    "provider": "anthropic",
    "model": "legacy/test-model-2",
    "modelParams": {},
    "ephemeralSettings": {
        "base-url": "https://api.anthropic.com",
        "context-limit": 200000,
        "max_tokens": 12000,
    },
    "auth": {
        "type": "oauth",
        "buckets": ["default"]
    }
}

(profiles_dir / 'legacy_test_profile_1.json').write_text(json.dumps(legacy_1, indent=2))
(profiles_dir / 'legacy_test_profile_2.json').write_text(json.dumps(legacy_2, indent=2))
print('wrote legacy profile fixtures')
