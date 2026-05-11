#!/bin/bash
# Miner event monitor — sends Telegram notifications for nonce events
LOG="/root/hash256-gpu-miner/miner.log"
CHECK_INTERVAL=2

# Track last processed line
LAST_LINE=$(wc -l < "$LOG" 2>/dev/null || echo 0)

while true; do
    CURRENT_LINE=$(wc -l < "$LOG" 2>/dev/null || echo 0)

    if [ "$CURRENT_LINE" -gt "$LAST_LINE" ]; then
        # Read new lines
        NEW_LINES=$(tail -n +$((LAST_LINE + 1)) "$LOG" | head -n $((CURRENT_LINE - LAST_LINE)))

        # Check for FOUND nonce
        if echo "$NEW_LINES" | grep -q "FOUND nonce"; then
            NONCE_INFO=$(echo "$NEW_LINES" | grep "FOUND nonce" | tail -1)
            TIMESTAMP=$(echo "$NONCE_INFO" | grep -oP '\[\K[0-9-]+ [0-9:]+' | head -1)
            NONCE=$(echo "$NONCE_INFO" | grep -oP 'nonce: \K[0-9]+')
            ELAPSED=$(echo "$NONCE_INFO" | grep -oP 'elapsed: \K[0-9.]+')

            # Send notification
            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"⚡ *Nonce Found!*\n\nNonce: \`${NONCE}\`\nTime: ${TIMESTAMP}\nMining time: ${ELAPSED}s\n\n⏳ Waiting for TX confirmation...\"
                }" 2>/dev/null
        fi

        # Check for TX broadcast success
        if echo "$NEW_LINES" | grep -q "TX broadcast:"; then
            TX_INFO=$(echo "$NEW_LINES" | grep "TX broadcast:" | tail -1)
            TX_HASH=$(echo "$TX_INFO" | grep -oP 'TX broadcast: 0x\K[0-9a-f]+')

            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"✅ *TX Broadcast Success!*\n\nHash: \`0x${TX_HASH}\`\n\n🔗 [View on Etherscan](https://etherscan.io/tx/0x${TX_HASH})\"
                }" 2>/dev/null
        fi

        # Check for Confirmed block
        if echo "$NEW_LINES" | grep -q "Confirmed block"; then
            CONFIRM_INFO=$(echo "$NEW_LINES" | grep "Confirmed block" | tail -1)
            BLOCK_NUM=$(echo "$CONFIRM_INFO" | grep -oP 'Confirmed block: \K[0-9]+')
            GAS_USED=$(echo "$CONFIRM_INFO" | grep -oP 'gas used: \K[0-9]+')

            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"🎉 *Block Confirmed!*\n\nBlock: #${BLOCK_NUM}\nGas used: ${GAS_USED}\n\n💰 Claim successful!\"
                }" 2>/dev/null
        fi

        # Check for broadcast error
        if echo "$NEW_LINES" | grep -q "Broadcast error"; then
            ERR_INFO=$(echo "$NEW_LINES" | grep "Broadcast error" | tail -1)
            ERR_MSG=$(echo "$ERR_INFO" | grep -oP 'Broadcast error: \K.*')

            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"❌ *TX Broadcast Failed!*\n\nError: \`${ERR_MSG}\`\n\nMiner will keep trying...\"
                }" 2>/dev/null
        fi

        # Check for TX failed
        if echo "$NEW_LINES" | grep -q "TX failed"; then
            FAIL_INFO=$(echo "$NEW_LINES" | grep "TX failed" | tail -1)
            FAIL_MSG=$(echo "$FAIL_INFO" | grep -oP 'TX failed: \K.*')

            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"❌ *TX Failed!*\n\nError: \`${FAIL_MSG}\`\n\nMiner continues mining...\"
                }" 2>/dev/null
        fi

        # Check for insufficient funds
        if echo "$NEW_LINES" | grep -q "insufficient funds"; then
            curl -s -X POST "http://127.0.0.1:5800/api/v1/message" \
                -H "Content-Type: application/json" \
                -d "{
                    \"platform\": \"telegram\",
                    \"message\": \"⚠️ *Insufficient Funds!*\n\nWallet balance too low for gas. Top up ETH needed.\"
                }" 2>/dev/null
        fi

        LAST_LINE=$CURRENT_LINE
    fi

    sleep $CHECK_INTERVAL
done
