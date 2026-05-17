#!/usr/bin/env python3
"""Prefill Gaia model catalog from Hugging Face API.

This script is intentionally dependency-light (stdlib only).
It can:
1) read model ids from an existing catalog YAML (base list), and/or
2) discover top/trending models from the HF API,
then generate a fresh Gaia-compatible catalog YAML.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any

HF_API_BASE = "https://huggingface.co/api"
USER_AGENT = "gaia-hf-prefill/0.1"

_ID_LINE_RE = re.compile(r'^\s*-\s*id:\s*["\']?([^"\']+)["\']?\s*$')
_PARAM_RE = re.compile(r"(\d+(?:\.\d+)?)\s*([bm])\b", re.IGNORECASE)
_PARAM_HINT_RE = re.compile(r"(\d+(?:[._]\d+)?)\s*b\b", re.IGNORECASE)


@dataclass
class CatalogEntry:
    id: str
    display_name: str
    family: str
    params_b: float
    categories: list[str]
    recommended_use: str
    min_vram_gb_fp16: int
    min_vram_gb_int8: int
    min_vram_gb_int4: int
    supports_vllm: bool
    supports_tgi: bool
    gated: bool


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate Gaia catalog/models.yaml from HF API metadata."
    )
    parser.add_argument(
        "--from-existing",
        default="catalog/models.yaml",
        help="Read model ids from existing catalog file (default: %(default)s).",
    )
    parser.add_argument(
        "--no-existing",
        action="store_true",
        help="Ignore --from-existing and only use --ids / --discover-limit.",
    )
    parser.add_argument(
        "--ids",
        nargs="*",
        default=[],
        help="Explicit model ids to include (space-separated).",
    )
    parser.add_argument(
        "--discover-limit",
        type=int,
        default=0,
        help="Discover top N text-generation models from HF API and include them.",
    )
    parser.add_argument(
        "--discover-sort",
        choices=["trending", "downloads"],
        default="trending",
        help="Discovery ranking mode (default: %(default)s).",
    )
    parser.add_argument(
        "--pipeline-tag",
        default="text-generation",
        choices=["text-generation"],
        help="Pipeline tag for discovery (fixed to text-generation).",
    )
    parser.add_argument(
        "--search",
        default="",
        help="Optional search query for discovery (HF API `search=` parameter).",
    )
    parser.add_argument(
        "--output",
        default="catalog/models.generated.yaml",
        help="Output YAML path (default: %(default)s).",
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("HF_TOKEN", ""),
        help="HF token (default: read HF_TOKEN env).",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=25.0,
        help="HTTP timeout in seconds (default: %(default)s).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print YAML to stdout instead of writing output file.",
    )
    parser.add_argument(
        "--allow-non-text-generation",
        action="store_true",
        help="Disable default text-generation filter.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    model_ids: list[str] = []
    if not args.no_existing and args.from_existing and os.path.exists(args.from_existing):
        model_ids.extend(read_model_ids_from_catalog(args.from_existing))
    model_ids.extend(args.ids)

    if args.discover_limit > 0:
        discovered = discover_model_ids(
            limit=args.discover_limit,
            pipeline_tag=args.pipeline_tag,
            discover_sort=args.discover_sort,
            search=args.search,
            token=args.token,
            timeout=args.timeout,
        )
        model_ids.extend(discovered)

    model_ids = deduplicate_keep_order([item.strip() for item in model_ids if item.strip()])
    if not model_ids:
        print(
            "No model ids found. Provide --ids, --discover-limit, or a valid --from-existing file.",
            file=sys.stderr,
        )
        return 2

    entries: list[CatalogEntry] = []
    errors: list[str] = []
    for index, model_id in enumerate(model_ids, 1):
        try:
            meta = fetch_model_metadata(
                model_id=model_id, token=args.token, timeout=args.timeout
            )
            if (
                not args.allow_non_text_generation
                and not is_text_generation_model(meta, fallback_id=model_id)
            ):
                print(
                    f"[{index:>3}/{len(model_ids)}] skip {model_id} -> not text-generation"
                )
                continue
            entries.append(build_catalog_entry(meta, fallback_id=model_id))
            print(f"[{index:>3}/{len(model_ids)}] ok  {model_id}")
        except Exception as exc:  # noqa: BLE001 - script robustness
            errors.append(f"{model_id}: {exc}")
            print(f"[{index:>3}/{len(model_ids)}] err {model_id} -> {exc}", file=sys.stderr)

    if not entries:
        print("No entries generated successfully.", file=sys.stderr)
        return 1

    entries.sort(key=lambda item: (item.params_b, item.id.lower()))
    yaml_output = render_catalog_yaml(entries)

    if args.dry_run:
        print(yaml_output)
    else:
        os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)
        with open(args.output, "w", encoding="utf-8") as handle:
            handle.write(yaml_output)
        print(f"\nWrote {len(entries)} models to {args.output}")

    if errors:
        print(f"\nCompleted with {len(errors)} errors:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)

    return 0


def read_model_ids_from_catalog(path: str) -> list[str]:
    ids: list[str] = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            match = _ID_LINE_RE.match(line)
            if match:
                ids.append(match.group(1).strip())
    return ids


def discover_model_ids(
    *,
    limit: int,
    pipeline_tag: str,
    discover_sort: str,
    search: str,
    token: str,
    timeout: float,
) -> list[str]:
    sort_field = "trendingScore" if discover_sort == "trending" else "downloads"
    params = {
        "limit": str(limit),
        "sort": sort_field,
        "direction": "-1",
        "pipeline_tag": pipeline_tag,
        "full": "true",
    }
    if search:
        params["search"] = search
    url = f"{HF_API_BASE}/models?{urllib.parse.urlencode(params)}"
    try:
        payload = http_get_json(url, token=token, timeout=timeout)
    except RuntimeError:
        if sort_field != "trendingScore":
            raise
        # Fallback if trending sort isn't available in the current HF API behavior.
        params["sort"] = "downloads"
        fallback_url = f"{HF_API_BASE}/models?{urllib.parse.urlencode(params)}"
        payload = http_get_json(fallback_url, token=token, timeout=timeout)
    output: list[str] = []
    if isinstance(payload, list):
        for item in payload:
            if isinstance(item, dict) and item.get("id"):
                output.append(str(item["id"]))
    return output


def fetch_model_metadata(*, model_id: str, token: str, timeout: float) -> dict[str, Any]:
    encoded = urllib.parse.quote(model_id, safe="/")
    url = f"{HF_API_BASE}/models/{encoded}?full=true"
    payload = http_get_json(url, token=token, timeout=timeout)
    if not isinstance(payload, dict):
        raise RuntimeError("Invalid model metadata payload from HF API.")
    return payload


def is_text_generation_model(meta: dict[str, Any], *, fallback_id: str) -> bool:
    pipeline_tag = str(meta.get("pipeline_tag") or "").strip().lower()
    tags = normalize_tags(meta.get("tags"))
    probe = " ".join([fallback_id.lower(), pipeline_tag] + tags)
    if contains_any_token(probe, ("gguf", "ggml")):
        return False

    if pipeline_tag == "text-generation":
        return True

    return contains_any_token(
        probe,
        (
            "text-generation",
            "text2text-generation",
            "conversational",
            "instruct",
            "chat",
            "causal-lm",
            "causal language model",
        ),
    )


def http_get_json(url: str, *, token: str, timeout: float) -> Any:
    headers = {
        "User-Agent": USER_AGENT,
        "Accept": "application/json",
    }
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"HTTP {exc.code} for {url} :: {body[:220]}") from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(f"Network error for {url}: {exc}") from exc


def build_catalog_entry(meta: dict[str, Any], *, fallback_id: str) -> CatalogEntry:
    model_id = str(meta.get("id") or fallback_id)
    tags = normalize_tags(meta.get("tags"))
    pipeline_tag = str(meta.get("pipeline_tag") or "").strip().lower()
    card_data = meta.get("cardData")
    if not isinstance(card_data, dict):
        card_data = {}

    params_b = estimate_params_b(model_id, tags, card_data)
    categories = infer_categories(model_id, tags, pipeline_tag, card_data, params_b)
    family = infer_family(model_id)
    display_name = infer_display_name(model_id, card_data)
    recommended_use = infer_recommended_use(categories)

    fp16, int8, int4 = estimate_vram_gb(params_b)
    supports_vllm, supports_tgi = infer_backend_support(categories, pipeline_tag, tags, model_id)
    gated = bool(meta.get("gated"))

    return CatalogEntry(
        id=model_id,
        display_name=display_name,
        family=family,
        params_b=params_b,
        categories=categories,
        recommended_use=recommended_use,
        min_vram_gb_fp16=fp16,
        min_vram_gb_int8=int8,
        min_vram_gb_int4=int4,
        supports_vllm=supports_vllm,
        supports_tgi=supports_tgi,
        gated=gated,
    )


def normalize_tags(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    output: list[str] = []
    for item in value:
        if item is None:
            continue
        tag = str(item).strip()
        if tag:
            output.append(tag.lower())
    return output


def infer_family(model_id: str) -> str:
    probe = model_id.lower()
    mapping = [
        ("qwen", "qwen"),
        ("mistral", "mistral"),
        ("llama", "llama"),
        ("gemma", "gemma"),
        ("deepseek", "deepseek"),
        ("phi", "phi"),
        ("mixtral", "mistral"),
        ("command-r", "cohere"),
        ("yi-", "yi"),
        ("olmo", "olmo"),
        ("gpt", "gpt"),
    ]
    for token, family in mapping:
        if token in probe:
            return family
    namespace = model_id.split("/", 1)[0].lower()
    return re.sub(r"[^a-z0-9\-_.]+", "-", namespace) or "unknown"


def infer_display_name(model_id: str, card_data: dict[str, Any]) -> str:
    for key in ("model_name", "name", "title"):
        value = card_data.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()

    slug = model_id.split("/", 1)[-1]
    words = re.split(r"[-_]+", slug)
    pretty = []
    for word in words:
        if not word:
            continue
        if any(char.isdigit() for char in word):
            pretty.append(word)
        else:
            pretty.append(word.capitalize())
    return " ".join(pretty) or slug


def estimate_params_b(model_id: str, tags: list[str], card_data: dict[str, Any]) -> float:
    candidates: list[float] = []
    for key in ("params", "parameter_count", "model_size", "size"):
        value = card_data.get(key)
        parsed = parse_params_value(value)
        if parsed:
            candidates.append(parsed)
    for tag in tags:
        parsed = parse_params_value(tag)
        if parsed:
            candidates.append(parsed)

    # fallback from model id
    probe = model_id.lower().replace("_", ".")
    for match in _PARAM_HINT_RE.finditer(probe):
        numeric = match.group(1).replace("_", ".")
        try:
            value = float(numeric)
            if 0.1 <= value <= 1000:
                candidates.append(value)
        except ValueError:
            continue

    if not candidates:
        return 7.0

    # keep plausible range and choose the smallest plausible "B" hint
    plausible = sorted(value for value in candidates if 0.1 <= value <= 5000)
    if not plausible:
        return 7.0
    return round(plausible[0], 1)


def parse_params_value(value: Any) -> float | None:
    if value is None:
        return None
    if isinstance(value, (int, float)):
        numeric = float(value)
        if numeric > 10000:
            # Probably raw parameter count.
            return numeric / 1_000_000_000.0
        return numeric
    if not isinstance(value, str):
        return None

    text = value.strip().lower().replace(",", "")
    match = _PARAM_RE.search(text)
    if not match:
        return None
    numeric = float(match.group(1))
    unit = match.group(2).lower()
    if unit == "m":
        return numeric / 1000.0
    return numeric


def infer_categories(
    model_id: str,
    tags: list[str],
    pipeline_tag: str,
    card_data: dict[str, Any],
    params_b: float,
) -> list[str]:
    probe = " ".join([model_id.lower(), pipeline_tag] + tags)
    categories: list[str] = []

    if contains_any_token(probe, ("instruct", "chat", "assistant", "text-generation")):
        categories.append("chat")
    if contains_any_token(probe, ("code", "coder", "programming")):
        categories.append("code")
    if contains_any_token(probe, ("reason", "r1", "math", "thinking", "think")):
        categories.append("reasoning")
    if infer_multilingual(tags, card_data, probe):
        categories.append("multilingual")
    if params_b <= 8.0:
        categories.append("lightweight")
    if contains_any_token(probe, ("32k", "64k", "128k", "long-context", "long context")):
        categories.append("long-context")
    if contains_any_token(
        probe,
        (
            "embedding",
            "feature-extraction",
            "sentence-similarity",
            "text-embeddings",
        ),
    ):
        categories.append("embeddings")
    if contains_any_token(
        probe,
        (
            "vision",
            "multimodal",
            "vl",
            "image-text-to-text",
            "llava",
        ),
    ):
        categories.append("vision/multimodal")

    if not categories:
        categories.append("general")

    order = [
        "chat",
        "code",
        "reasoning",
        "multilingual",
        "lightweight",
        "long-context",
        "embeddings",
        "vision/multimodal",
        "general",
    ]
    deduped = deduplicate_keep_order(categories)
    deduped.sort(key=lambda item: order.index(item) if item in order else len(order))
    return deduped


def infer_multilingual(tags: list[str], card_data: dict[str, Any], probe: str) -> bool:
    if contains_any_token(probe, ("multilingual",)):
        return True
    languages = extract_languages(tags, card_data)
    return len(languages) >= 2


def extract_languages(tags: list[str], card_data: dict[str, Any]) -> set[str]:
    languages: set[str] = set()
    language_meta = card_data.get("language")
    if isinstance(language_meta, str) and language_meta.strip():
        languages.add(language_meta.strip().lower())
    elif isinstance(language_meta, list):
        for item in language_meta:
            if isinstance(item, str) and item.strip():
                languages.add(item.strip().lower())

    for tag in tags:
        if tag.startswith("language:"):
            languages.add(tag.split(":", 1)[1].strip().lower())
        elif tag in {"en", "fr", "de", "es", "it", "pt", "zh", "ja", "ko", "ar", "ru"}:
            languages.add(tag)
    return languages


def infer_recommended_use(categories: list[str]) -> str:
    if "embeddings" in categories:
        return "Text embeddings and semantic search"
    if "vision/multimodal" in categories:
        return "Vision and multimodal assistant tasks"
    if "code" in categories and "chat" in categories:
        return "Coding assistant and developer chat"
    if "reasoning" in categories:
        return "Reasoning-focused assistant"
    if "chat" in categories:
        return "General-purpose assistant"
    return "General language model inference"


def estimate_vram_gb(params_b: float) -> tuple[int, int, int]:
    fp16 = max(4, int(math.ceil(params_b * 2.0 + 2.0)))
    int8 = max(3, int(math.ceil(fp16 * 0.62)))
    int4 = max(2, int(math.ceil(fp16 * 0.37)))
    return fp16, int8, int4


def infer_backend_support(
    categories: list[str], pipeline_tag: str, tags: list[str], model_id: str
) -> tuple[bool, bool]:
    probe = " ".join([model_id.lower(), pipeline_tag] + tags)
    if contains_any_token(probe, ("gguf",)):
        return False, False
    if "embeddings" in categories:
        return False, False
    if "vision/multimodal" in categories:
        return False, False
    if contains_any_token(
        probe,
        (
            "text-generation",
            "conversational",
            "text2text-generation",
            "instruct",
            "chat",
        ),
    ):
        return True, True
    return False, False


def contains_any_token(probe: str, keywords: tuple[str, ...]) -> bool:
    return any(contains_token(probe, keyword) for keyword in keywords)


def contains_token(probe: str, keyword: str) -> bool:
    key = keyword.lower()
    if re.fullmatch(r"[a-z0-9]+", key):
        pattern = rf"(?<![a-z0-9]){re.escape(key)}(?![a-z0-9])"
    else:
        pattern = re.escape(key)
    return re.search(pattern, probe) is not None


def render_catalog_yaml(entries: list[CatalogEntry]) -> str:
    lines = ["models:"]
    for item in entries:
        lines.append(f'  - id: "{yaml_escape(item.id)}"')
        lines.append(f'    display_name: "{yaml_escape(item.display_name)}"')
        lines.append(f'    family: "{yaml_escape(item.family)}"')
        lines.append(f"    params_b: {format_float(item.params_b)}")
        lines.append(
            "    categories: ["
            + ", ".join(f'"{yaml_escape(category)}"' for category in item.categories)
            + "]"
        )
        lines.append(f'    recommended_use: "{yaml_escape(item.recommended_use)}"')
        lines.append(f"    min_vram_gb_fp16: {item.min_vram_gb_fp16}")
        lines.append(f"    min_vram_gb_int8: {item.min_vram_gb_int8}")
        lines.append(f"    min_vram_gb_int4: {item.min_vram_gb_int4}")
        lines.append(f"    supports_vllm: {str(item.supports_vllm).lower()}")
        lines.append(f"    supports_tgi: {str(item.supports_tgi).lower()}")
        lines.append(f"    gated: {str(item.gated).lower()}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def yaml_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def format_float(value: float) -> str:
    if float(value).is_integer():
        return str(int(value))
    return f"{value:.1f}"


def deduplicate_keep_order(items: list[str]) -> list[str]:
    seen: set[str] = set()
    output: list[str] = []
    for item in items:
        if item not in seen:
            seen.add(item)
            output.append(item)
    return output


if __name__ == "__main__":
    raise SystemExit(main())
