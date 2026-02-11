cd /home/lipeng/nebula/frontend
export PATH=/home/lipeng/node/bin:$PATH
pkill -u lipeng -f vite || true
nohup node node_modules/vite/bin/vite.js --host --port 5173 > ../logs/frontend_premium_final_v6.log 2>&1 &
