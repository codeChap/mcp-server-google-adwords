#!/bin/bash
#
# Integration test for the Google Ads MCP server via stdio.
# Requires a valid config at ~/.config/mcp-server-google-adwords/config.toml
# with working OAuth tokens (run get-token first).
#
# Usage:
#   ./tests/test-stdio.sh            # run all tests
#   ./tests/test-stdio.sh --init     # only test initialize + tools/list (no API calls)

set -euo pipefail
cd "$(dirname "$0")/.."

BINARY="./target/release/google-adwords-mcp"
INIT_ONLY=false

if [[ "${1:-}" == "--init" ]]; then
    INIT_ONLY=true
fi

if [[ ! -f "$BINARY" ]]; then
    echo "Building release binary..."
    cargo build --release
fi

# Colours
GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

pass() { echo -e "  ${GREEN}✓${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; FAILURES=$((FAILURES + 1)); }
section() { echo -e "\n${CYAN}── $1 ──${NC}"; }

FAILURES=0

# ---------------------------------------------------------------------------
# Helper: send JSON-RPC messages and capture output
# ---------------------------------------------------------------------------
run_mcp() {
    echo "$1" | "$BINARY" 2>/dev/null
}

# ---------------------------------------------------------------------------
# Test 1: Initialize + tools/list (no Google Ads credentials needed for parse)
# ---------------------------------------------------------------------------
section "MCP Protocol"

INIT_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}')

# Check server name in initialize response
if echo "$INIT_RESPONSE" | grep -q '"google-adwords-mcp"'; then
    pass "initialize returns server name"
else
    fail "initialize missing server name"
fi

# Check all 8 tools are listed
EXPECTED_TOOLS=("list_campaigns" "create_campaign" "update_campaign_status" "get_campaign_stats" "list_budgets" "create_budget" "update_budget" "search")
for tool in "${EXPECTED_TOOLS[@]}"; do
    if echo "$INIT_RESPONSE" | grep -q "\"$tool\""; then
        pass "tools/list includes $tool"
    else
        fail "tools/list missing $tool"
    fi
done

if $INIT_ONLY; then
    section "Results"
    if [[ $FAILURES -eq 0 ]]; then
        echo -e "${GREEN}All tests passed.${NC}"
    else
        echo -e "${RED}$FAILURES test(s) failed.${NC}"
    fi
    exit $FAILURES
fi

# ---------------------------------------------------------------------------
# Test 2: list_campaigns (requires valid credentials)
# ---------------------------------------------------------------------------
section "Live API (requires credentials)"

LIST_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_campaigns","arguments":{"limit":5}}}')

if echo "$LIST_RESPONSE" | grep -q '"result"'; then
    pass "list_campaigns returns a result"
else
    fail "list_campaigns failed (check credentials)"
fi

# ---------------------------------------------------------------------------
# Test 3: list_budgets
# ---------------------------------------------------------------------------
BUDGET_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_budgets","arguments":{"limit":5}}}')

if echo "$BUDGET_RESPONSE" | grep -q '"result"'; then
    pass "list_budgets returns a result"
else
    fail "list_budgets failed"
fi

# ---------------------------------------------------------------------------
# Test 4: search with raw GAQL
# ---------------------------------------------------------------------------
SEARCH_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"search","arguments":{"query":"SELECT campaign.id, campaign.name FROM campaign LIMIT 1"}}}')

if echo "$SEARCH_RESPONSE" | grep -q '"result"'; then
    pass "search (raw GAQL) returns a result"
else
    fail "search (raw GAQL) failed"
fi

# ---------------------------------------------------------------------------
# Test 5: validation — bad campaign_id should return error content, not crash
# ---------------------------------------------------------------------------
section "Validation"

BAD_ID_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"update_campaign_status","arguments":{"campaign_id":"not-a-number","status":"PAUSED"}}}')

if echo "$BAD_ID_RESPONSE" | grep -q 'numeric string'; then
    pass "update_campaign_status rejects non-numeric campaign_id"
else
    fail "update_campaign_status did not reject bad campaign_id"
fi

BAD_STATUS_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"update_campaign_status","arguments":{"campaign_id":"123","status":"INVALID"}}}')

if echo "$BAD_STATUS_RESPONSE" | grep -q 'Invalid status'; then
    pass "update_campaign_status rejects invalid status"
else
    fail "update_campaign_status did not reject bad status"
fi

EMPTY_QUERY_RESPONSE=$(run_mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","method":"initialized"}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"search","arguments":{"query":"  "}}}')

if echo "$EMPTY_QUERY_RESPONSE" | grep -q 'must not be empty'; then
    pass "search rejects empty query"
else
    fail "search did not reject empty query"
fi

# ---------------------------------------------------------------------------
# Results
# ---------------------------------------------------------------------------
section "Results"
if [[ $FAILURES -eq 0 ]]; then
    echo -e "${GREEN}All tests passed.${NC}"
else
    echo -e "${RED}$FAILURES test(s) failed.${NC}"
fi
exit $FAILURES
