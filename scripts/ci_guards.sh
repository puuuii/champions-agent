#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

ERRORS=0

echo "=== Forbidden Dependency Guard ==="

echo "  Checking champions-domain..."
if cargo tree -p champions-domain 2>/dev/null | grep -qE "opencv|iced|ort|manga-ocr|reqwest|csv"; then
    echo "  FAIL: champions-domain has forbidden dependencies"
    cargo tree -p champions-domain | grep -E "opencv|iced|ort|manga-ocr|reqwest|csv"
    ERRORS=$((ERRORS + 1))
else
    echo "  OK"
fi

echo "  Checking champions-application..."
if cargo tree -p champions-application 2>/dev/null | grep -qE "champions-interface|opencv|iced|ort|manga-ocr|reqwest"; then
    echo "  FAIL: champions-application has forbidden dependencies"
    cargo tree -p champions-application | grep -E "champions-interface|opencv|iced|ort|manga-ocr|reqwest"
    ERRORS=$((ERRORS + 1))
else
    echo "  OK"
fi

echo "  Checking champions-runtime..."
if cargo tree -p champions-runtime 2>/dev/null | grep -qE "champions-infrastructure|opencv|iced|ort|manga-ocr"; then
    echo "  FAIL: champions-runtime has forbidden dependencies"
    cargo tree -p champions-runtime | grep -E "champions-infrastructure|opencv|iced|ort|manga-ocr"
    ERRORS=$((ERRORS + 1))
else
    echo "  OK"
fi

echo ""
echo "=== UI Import Guard ==="

UI_FILES=(
    "apps/desktop/src/ui/app.rs"
    "apps/desktop/src/ui/pokemon.rs"
    "apps/desktop/src/ui/subscriptions.rs"
    "apps/desktop/src/ui/components.rs"
    "apps/desktop/src/ui/components/video_preview.rs"
)

for f in "${UI_FILES[@]}"; do
    if [ -f "$f" ] && grep -q "champions_infrastructure" "$f"; then
        echo "  FAIL: $f imports champions_infrastructure"
        ERRORS=$((ERRORS + 1))
    fi
done

if [ $ERRORS -eq 0 ]; then
    echo "  OK: No UI files import champions_infrastructure"
fi

echo ""
echo "=== Mat Leak Guard ==="

MAT_LEAK_DIRS=(
    "crates/champions-domain"
    "crates/champions-application"
    "crates/champions-interface"
    "crates/champions-runtime"
)

for dir in "${MAT_LEAK_DIRS[@]}"; do
    if grep -rq "opencv::core::Mat\|core::Mat\|prelude::.*Mat" "$dir" 2>/dev/null; then
        echo "  FAIL: $dir contains Mat references"
        grep -rn "opencv::core::Mat\|core::Mat\|prelude::.*Mat" "$dir"
        ERRORS=$((ERRORS + 1))
    fi
done

if [ $ERRORS -eq 0 ]; then
    echo "  OK: No Mat leaks detected"
fi

echo ""
echo "=== Results ==="
if [ $ERRORS -gt 0 ]; then
    echo "FAILED: $ERRORS guard(s) failed"
    exit 1
else
    echo "ALL GUARDS PASSED"
    exit 0
fi
