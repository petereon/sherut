#!/bin/bash
set -e

# Configuration
PORT=8080
HOST="localhost"
BASE_URL="http://${HOST}:${PORT}"
WARMUP=3
RUNS=100
TEST_ID=1

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Cleanup function
cleanup() {
    echo -e "${BLUE}Cleaning up...${NC}"
    pkill -f "uvicorn main:app" 2>/dev/null || true
    pkill -f "sherut.*--port.*$PORT" 2>/dev/null || true
    sleep 0.5
}

trap cleanup EXIT

# Wait for server to be ready
wait_for_server() {
    local max_attempts=50
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/people/1" 2>/dev/null | grep -qE "200|404"; then
            return 0
        fi
        sleep 0.2
        attempt=$((attempt + 1))
    done
    
    echo "Server failed to start" >&2
    return 1
}

# Check dependencies
check_dependencies() {
    if ! command -v hyperfine &> /dev/null; then
        echo -e "${RED}Error: hyperfine is not installed. Install with: brew install hyperfine${NC}"
        exit 1
    fi
    
    if ! command -v uvicorn &> /dev/null; then
        echo -e "${RED}Error: uvicorn is not installed. Run: pip install -r requirements.txt${NC}"
        exit 1
    fi
    
    if ! command -v curl &> /dev/null; then
        echo -e "${RED}Error: curl is not installed${NC}"
        exit 1
    fi
}

# Benchmark FastAPI
benchmark_fastapi() {
    echo -e "\n${GREEN}=== Benchmarking FastAPI ===${NC}\n"
    
    cd "$SCRIPT_DIR"
    
    # Start FastAPI server
    uvicorn main:app --host $HOST --port $PORT --log-level warning &
    FASTAPI_PID=$!
    
    if ! wait_for_server; then
        echo -e "${RED}FastAPI failed to start${NC}"
        return 1
    fi
    
    echo -e "${BLUE}FastAPI server ready on ${BASE_URL}${NC}\n"
    
    # Run benchmark
    hyperfine \
        --warmup $WARMUP \
        --runs $RUNS \
        --export-json fastapi_results.json \
        "curl -s ${BASE_URL}/people/${TEST_ID}"
    
    # Stop server
    kill $FASTAPI_PID 2>/dev/null || true
    wait $FASTAPI_PID 2>/dev/null || true
    
    echo -e "\n${GREEN}FastAPI benchmark complete${NC}"
}

# Benchmark Sherut
benchmark_sherut() {
    echo -e "\n${GREEN}=== Benchmarking Sherut ===${NC}\n"
    
    cd "$PROJECT_DIR"
    
    # Build sherut first
    echo -e "${BLUE}Building sherut...${NC}"
    cargo build --release --quiet
    
    # Start sherut server
    ./target/release/sherut \
        --port $PORT \
        --route "GET /people/:id" "${SCRIPT_DIR}/people.sh :id" &
    SHERUT_PID=$!
    
    if ! wait_for_server; then
        echo -e "${RED}Sherut failed to start${NC}"
        return 1
    fi
    
    echo -e "${BLUE}Sherut server ready on ${BASE_URL}${NC}\n"
    
    # Run benchmark  
    hyperfine \
        --warmup $WARMUP \
        --runs $RUNS \
        --export-json "${SCRIPT_DIR}/sherut_results.json" \
        "curl -s ${BASE_URL}/people/${TEST_ID}"
    
    # Stop server
    kill $SHERUT_PID 2>/dev/null || true
    wait $SHERUT_PID 2>/dev/null || true
    
    echo -e "\n${GREEN}Sherut benchmark complete${NC}"
}

# Compare results
compare_results() {
    echo -e "\n${GREEN}=== Comparison ===${NC}\n"
    
    cd "$SCRIPT_DIR"
    
    if [ -f fastapi_results.json ] && [ -f sherut_results.json ]; then
        FASTAPI_MEAN=$(jq '.results[0].mean' fastapi_results.json)
        SHERUT_MEAN=$(jq '.results[0].mean' sherut_results.json)
        
        FASTAPI_MS=$(echo "$FASTAPI_MEAN * 1000" | bc -l | xargs printf "%.2f")
        SHERUT_MS=$(echo "$SHERUT_MEAN * 1000" | bc -l | xargs printf "%.2f")
        
        echo "FastAPI: ${FASTAPI_MS}ms (mean)"
        echo "Sherut:  ${SHERUT_MS}ms (mean)"
        
        if (( $(echo "$FASTAPI_MEAN > $SHERUT_MEAN" | bc -l) )); then
            RATIO=$(echo "$FASTAPI_MEAN / $SHERUT_MEAN" | bc -l | xargs printf "%.2f")
            echo -e "\n${GREEN}Sherut is ${RATIO}x faster than FastAPI${NC}"
        else
            RATIO=$(echo "$SHERUT_MEAN / $FASTAPI_MEAN" | bc -l | xargs printf "%.2f")
            echo -e "\n${GREEN}FastAPI is ${RATIO}x faster than Sherut${NC}"
        fi
    fi
}

# Main
main() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Sherut vs FastAPI Benchmark${NC}"
    echo -e "${BLUE}========================================${NC}"
    
    check_dependencies
    cleanup
    
    benchmark_fastapi
    cleanup
    
    benchmark_sherut
    cleanup
    
    compare_results
    
    echo -e "\n${GREEN}Benchmark complete!${NC}"
}

# Allow running individual benchmarks
case "${1:-all}" in
    fastapi)
        check_dependencies
        cleanup
        benchmark_fastapi
        ;;
    sherut)
        check_dependencies
        cleanup
        benchmark_sherut
        ;;
    compare)
        compare_results
        ;;
    all|*)
        main
        ;;
esac
