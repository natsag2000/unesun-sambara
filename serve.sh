@echo off
echo Starting HTTP server on http://localhost:8080
python -m http.server 8080 --bind 0.0.0.0
