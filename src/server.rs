use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub struct ServerConfig {
    pub gateway: String,
    pub port: u16,
}

pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let state = Arc::new(config);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/control", get(control_proxy))
        .route("/stream", get(stream_proxy))
        .layer(cors)
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", state.port);
    println!("Starting server at http://localhost:{}", state.port);
    println!("Proxying to gateway: {}", state.gateway);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> Html<String> {
    // Stream is proxied through /stream endpoint
    Html(generate_html("/stream"))
}

async fn control_proxy(
    State(config): State<Arc<ServerConfig>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let query_string: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    let url = format!("http://{}/control?{}", config.gateway, query_string);

    match ureq::get(&url).call() {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            (StatusCode::OK, body)
        }
        Err(e) => (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)),
    }
}

async fn stream_proxy(State(config): State<Arc<ServerConfig>>) -> Response {
    let stream_url = format!("http://{}:81/stream", config.gateway);

    let client = reqwest::Client::new();
    match client.get(&stream_url).send().await {
        Ok(response) => {
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("multipart/x-mixed-replace; boundary=frame")
                .to_string();

            let stream = response.bytes_stream();

            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", content_type)
                .body(Body::from_stream(stream))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(format!("Stream error: {}", e)))
            .unwrap(),
    }
}

fn generate_html(stream_url: &str) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Robot Dog Control</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        .btn-press:active {{ transform: scale(0.95); }}
        .key-active {{ background-color: #10b981 !important; transform: scale(0.95); }}
        .gamepad-status {{ position: fixed; top: 10px; right: 10px; padding: 6px 12px; border-radius: 4px; font-size: 12px; z-index: 1000; }}
        .gamepad-connected {{ background: #166534; color: #fff; }}
        .gamepad-disconnected {{ background: #374151; color: #6b7280; }}
    </style>
</head>
<body class="bg-gray-900 text-white min-h-screen">
    <div id="gamepad-status" class="gamepad-status gamepad-disconnected">Gamepad: Press any button</div>
    <div class="container mx-auto px-4 py-8 max-w-4xl">
        <header class="text-center mb-6">
            <h1 class="text-2xl font-bold text-cyan-400">ESP32 Robot Dog Control</h1>
            <p class="text-gray-500 text-sm">WASD/Arrows to move | Space = Stop | Gamepad supported</p>
        </header>

        <!-- Camera Stream -->
        <div class="bg-gray-800 rounded-xl p-3 mb-4 shadow-lg">
            <div class="flex justify-between items-center mb-2">
                <h2 class="text-sm font-semibold text-gray-300">Camera</h2>
                <button id="toggle-stream"
                    class="btn-press px-3 py-1 bg-cyan-600 hover:bg-cyan-500 rounded text-sm font-medium transition-colors">
                    Start
                </button>
            </div>
            <div id="stream-container" class="relative bg-black rounded-lg overflow-hidden flex justify-center">
                <img id="stream" src="" class="hidden max-w-[320px] h-auto" alt="Camera stream">
                <div id="stream-placeholder" class="flex items-center justify-center h-32 w-full text-gray-500 text-sm">
                    Click "Start" to view camera
                </div>
            </div>
        </div>

        <!-- Movement Controls -->
        <div class="bg-gray-800 rounded-xl p-4 mb-4 shadow-lg">
            <h2 class="text-sm font-semibold text-gray-300 mb-3 text-center">Movement (Arrow Keys)</h2>
            <div class="flex flex-col items-center gap-1">
                <button id="forward"
                    class="btn-press w-20 h-10 bg-emerald-600 hover:bg-emerald-500 rounded-lg font-bold text-sm transition-colors flex items-center justify-center">
                    <span class="mr-1">W</span>
                </button>
                <div class="flex gap-1">
                    <button id="left"
                        class="btn-press w-20 h-10 bg-emerald-600 hover:bg-emerald-500 rounded-lg font-bold text-sm transition-colors flex items-center justify-center">
                        <span class="mr-1">A</span>
                    </button>
                    <button id="steady"
                        class="btn-press w-20 h-10 bg-blue-600 hover:bg-blue-500 rounded-lg font-bold text-sm transition-colors">
                        STOP
                    </button>
                    <button id="right"
                        class="btn-press w-20 h-10 bg-emerald-600 hover:bg-emerald-500 rounded-lg font-bold text-sm transition-colors flex items-center justify-center">
                        <span class="mr-1">D</span>
                    </button>
                </div>
                <button id="backward"
                    class="btn-press w-20 h-10 bg-emerald-600 hover:bg-emerald-500 rounded-lg font-bold text-sm transition-colors flex items-center justify-center">
                    <span class="mr-1">S</span>
                </button>
            </div>
        </div>

        <!-- Actions -->
        <div class="bg-gray-800 rounded-xl p-6 mb-6 shadow-lg">
            <h2 class="text-lg font-semibold text-gray-200 mb-4">Actions</h2>
            <div class="grid grid-cols-3 gap-3">
                <button onclick="sendAction(2)"
                    class="btn-press py-3 bg-purple-600 hover:bg-purple-500 rounded-lg font-medium transition-colors">
                    Stay Low
                </button>
                <button onclick="sendAction(3)"
                    class="btn-press py-3 bg-purple-600 hover:bg-purple-500 rounded-lg font-medium transition-colors">
                    Hand Shake
                </button>
                <button onclick="sendAction(4)"
                    class="btn-press py-3 bg-purple-600 hover:bg-purple-500 rounded-lg font-medium transition-colors">
                    Jump
                </button>
                <button onclick="sendAction(5)"
                    class="btn-press py-3 bg-indigo-600 hover:bg-indigo-500 rounded-lg font-medium transition-colors">
                    Action A
                </button>
                <button onclick="sendAction(6)"
                    class="btn-press py-3 bg-indigo-600 hover:bg-indigo-500 rounded-lg font-medium transition-colors">
                    Action B
                </button>
                <button onclick="sendAction(7)"
                    class="btn-press py-3 bg-indigo-600 hover:bg-indigo-500 rounded-lg font-medium transition-colors">
                    Action C
                </button>
            </div>
        </div>

        <!-- Position Presets -->
        <div class="bg-gray-800 rounded-xl p-6 mb-6 shadow-lg">
            <h2 class="text-lg font-semibold text-gray-200 mb-4">Position Presets</h2>
            <div class="flex justify-center gap-4">
                <button onclick="sendAction(8)"
                    class="btn-press px-6 py-3 bg-orange-600 hover:bg-orange-500 rounded-lg font-medium transition-colors">
                    Init Position
                </button>
                <button onclick="sendAction(9)"
                    class="btn-press px-6 py-3 bg-orange-600 hover:bg-orange-500 rounded-lg font-medium transition-colors">
                    Middle Position
                </button>
            </div>
        </div>

        <!-- Servo Controls -->
        <div class="bg-gray-800 rounded-xl p-6 shadow-lg">
            <h2 class="text-lg font-semibold text-gray-200 mb-4">Servo Controls (PWM 0-15)</h2>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-3" id="servo-controls">
            </div>
        </div>

        <footer class="text-center mt-8 text-gray-500 text-sm">
            <p>WiFi Proxy - Connected to gateway</p>
        </footer>
    </div>

    <script>
        const STREAM_URL = "{}";

        // Stream toggle
        const streamImg = document.getElementById('stream');
        const streamBtn = document.getElementById('toggle-stream');
        const placeholder = document.getElementById('stream-placeholder');
        let streaming = false;

        streamBtn.onclick = () => {{
            if (streaming) {{
                streamImg.src = '';
                streamImg.classList.add('hidden');
                placeholder.classList.remove('hidden');
                streamBtn.textContent = 'Start Stream';
                streaming = false;
            }} else {{
                streamImg.src = STREAM_URL;
                streamImg.classList.remove('hidden');
                placeholder.classList.add('hidden');
                streamBtn.textContent = 'Stop Stream';
                streaming = true;
            }}
        }};

        // Movement controls
        function sendMove(val) {{
            fetch(`/control?var=move&val=${{val}}&cmd=0`);
        }}

        function sendAction(val) {{
            fetch(`/control?var=funcMode&val=${{val}}&cmd=0`);
        }}

        function sendServo(servo, delta) {{
            fetch(`/control?var=sconfig&val=${{servo}}&cmd=${{delta}}`);
        }}

        function setServo(servo) {{
            fetch(`/control?var=sset&val=${{servo}}&cmd=1`);
        }}

        // Movement button events
        const movements = {{
            forward: {{ down: 1, up: 3 }},
            backward: {{ down: 5, up: 3 }},
            left: {{ down: 2, up: 6 }},
            right: {{ down: 4, up: 6 }}
        }};

        Object.entries(movements).forEach(([id, vals]) => {{
            const btn = document.getElementById(id);
            ['mousedown', 'touchstart'].forEach(evt => {{
                btn.addEventListener(evt, (e) => {{ e.preventDefault(); sendMove(vals.down); }});
            }});
            ['mouseup', 'touchend', 'mouseleave'].forEach(evt => {{
                btn.addEventListener(evt, (e) => {{ e.preventDefault(); sendMove(vals.up); }});
            }});
        }});

        document.getElementById('steady').onclick = () => sendAction(1);

        // Keyboard controls
        const keyMap = {{
            'ArrowUp': 'forward',
            'ArrowDown': 'backward',
            'ArrowLeft': 'left',
            'ArrowRight': 'right',
            'w': 'forward',
            'W': 'forward',
            's': 'backward',
            'S': 'backward',
            'a': 'left',
            'A': 'left',
            'd': 'right',
            'D': 'right'
        }};

        const activeKeys = new Set();

        document.addEventListener('keydown', (e) => {{
            if (e.key === ' ' || e.key === 'Escape') {{
                e.preventDefault();
                sendAction(1); // Steady
                return;
            }}

            const btnId = keyMap[e.key];
            if (btnId && !activeKeys.has(e.key)) {{
                e.preventDefault();
                activeKeys.add(e.key);
                const btn = document.getElementById(btnId);
                btn.classList.add('key-active');
                sendMove(movements[btnId].down);
            }}
        }});

        document.addEventListener('keyup', (e) => {{
            const btnId = keyMap[e.key];
            if (btnId && activeKeys.has(e.key)) {{
                e.preventDefault();
                activeKeys.delete(e.key);
                const btn = document.getElementById(btnId);
                btn.classList.remove('key-active');
                sendMove(movements[btnId].up);
            }}
        }});

        // Generate servo controls
        const servoContainer = document.getElementById('servo-controls');
        for (let i = 0; i < 16; i++) {{
            const div = document.createElement('div');
            div.className = 'flex items-center gap-1 bg-gray-700 rounded-lg p-2';
            div.innerHTML = `
                <span class="text-xs text-gray-400 w-8">S${{i}}</span>
                <button onclick="sendServo(${{i}}, -1)"
                    class="btn-press flex-1 py-1 bg-red-600 hover:bg-red-500 rounded text-sm font-bold">-</button>
                <button onclick="sendServo(${{i}}, 1)"
                    class="btn-press flex-1 py-1 bg-green-600 hover:bg-green-500 rounded text-sm font-bold">+</button>
                <button onclick="setServo(${{i}})"
                    class="btn-press flex-1 py-1 bg-gray-600 hover:bg-gray-500 rounded text-xs">SET</button>
            `;
            servoContainer.appendChild(div);
        }}

        // Gamepad support
        const gamepadStatus = document.getElementById('gamepad-status');
        let gamepadIndex = null;
        let gpState = {{ forward: false, backward: false, left: false, right: false, buttons: {{}} }};

        const updateGamepadStatus = (connected, name = '') => {{
            if (connected) {{
                gamepadStatus.textContent = 'Gamepad: ' + (name.length > 25 ? name.substring(0, 25) + '...' : name);
                gamepadStatus.className = 'gamepad-status gamepad-connected';
            }} else {{
                gamepadStatus.textContent = 'Gamepad: Press any button';
                gamepadStatus.className = 'gamepad-status gamepad-disconnected';
            }}
        }};

        window.addEventListener('gamepadconnected', (e) => {{
            gamepadIndex = e.gamepad.index;
            updateGamepadStatus(true, e.gamepad.id);
            console.log('Gamepad connected:', e.gamepad.id);
        }});

        window.addEventListener('gamepaddisconnected', (e) => {{
            if (e.gamepad.index === gamepadIndex) {{
                gamepadIndex = null;
                updateGamepadStatus(false);
                sendMove(3); sendMove(6);
                gpState = {{ forward: false, backward: false, left: false, right: false, buttons: {{}} }};
            }}
        }});

        const AXIS_THRESHOLD = 0.5;

        const pollGamepad = () => {{
            if (gamepadIndex === null) {{
                requestAnimationFrame(pollGamepad);
                return;
            }}

            const gp = navigator.getGamepads()[gamepadIndex];
            if (!gp) {{
                requestAnimationFrame(pollGamepad);
                return;
            }}

            const leftX = gp.axes[0] || 0;
            const leftY = gp.axes[1] || 0;
            const dpadUp = gp.buttons[12]?.pressed || false;
            const dpadDown = gp.buttons[13]?.pressed || false;
            const dpadLeft = gp.buttons[14]?.pressed || false;
            const dpadRight = gp.buttons[15]?.pressed || false;

            const wantForward = leftY < -AXIS_THRESHOLD || dpadUp;
            const wantBackward = leftY > AXIS_THRESHOLD || dpadDown;
            const wantLeft = leftX < -AXIS_THRESHOLD || dpadLeft;
            const wantRight = leftX > AXIS_THRESHOLD || dpadRight;

            if (wantForward && !gpState.forward) {{ sendMove(1); gpState.forward = true; }}
            else if (!wantForward && gpState.forward) {{ sendMove(3); gpState.forward = false; }}

            if (wantBackward && !gpState.backward) {{ sendMove(5); gpState.backward = true; }}
            else if (!wantBackward && gpState.backward) {{ sendMove(3); gpState.backward = false; }}

            if (wantLeft && !gpState.left) {{ sendMove(2); gpState.left = true; }}
            else if (!wantLeft && gpState.left) {{ sendMove(6); gpState.left = false; }}

            if (wantRight && !gpState.right) {{ sendMove(4); gpState.right = true; }}
            else if (!wantRight && gpState.right) {{ sendMove(6); gpState.right = false; }}

            const handleBtn = (idx, action) => {{
                const pressed = gp.buttons[idx]?.pressed || false;
                if (pressed && !gpState.buttons[idx]) action();
                gpState.buttons[idx] = pressed;
            }};

            handleBtn(0, () => sendAction(1));  // A: Steady
            handleBtn(1, () => sendAction(2));  // B: Stay Low
            handleBtn(2, () => sendAction(3));  // X: Hand Shake
            handleBtn(3, () => sendAction(4));  // Y: Jump
            handleBtn(4, () => {{ if (!streaming) streamBtn.click(); }}); // LB: Camera ON
            handleBtn(5, () => {{ if (streaming) streamBtn.click(); }});  // RB: Camera OFF
            handleBtn(6, () => sendAction(5));  // LT: Action A
            handleBtn(7, () => sendAction(6));  // RT: Action B
            handleBtn(8, () => sendAction(7));  // Select: Action C
            handleBtn(9, () => sendAction(8));  // Start: Init Pos

            requestAnimationFrame(pollGamepad);
        }};

        requestAnimationFrame(pollGamepad);
    </script>
</body>
</html>"##,
        stream_url
    )
}
