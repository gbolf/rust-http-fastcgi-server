const demos = {
 static: { title: 'Statički sadržaj', method: 'GET', url: '/primjer.txt', },
 head: { title: 'HEAD — zaglavlja bez tijela', method: 'HEAD', url: '/primjer.txt', },
 get: { title: 'PHP GET — parametri u URL-u', method: 'GET', url: '/hello.php', name: 'Ivan' },
 post: { title: 'PHP POST — podaci obrasca', method: 'POST', url: '/form.php', name: 'Ana' },
 errors: { title: 'HTTP pogreške', method: 'GET', url: '/ne-postoji.txt', }
};
const panel = document.querySelector('#panel');
let generation = 0;
function render(key) {
 const current = ++generation;
 document.querySelectorAll('[data-demo]').forEach(button => { if (button.dataset.demo === key) button.setAttribute('aria-current', 'page'); else button.removeAttribute('aria-current'); });
 if (key === 'overview') {
 panel.innerHTML = `<h2>Pregled testova</h2><div class="test-list"><div><strong>Statički sadržaj</strong><span>GET · čitanje tekstualne datoteke</span></div><div><strong>HEAD</strong><span>Zaglavlja bez tijela odgovora</span></div><div><strong>PHP GET</strong><span>Slanje imena u URL-u</span></div><div><strong>PHP POST</strong><span>Slanje imena kroz obrazac</span></div><div><strong>Pogreške</strong><span>404 · 405 · 403</span></div></div>`;
 return;
 }
 const demo = demos[key];
 panel.innerHTML = `<h2>${demo.title}</h2><div class="demo-grid"><form id="demo-form">${demo.name ? `<label for="name">Ime za demonstraciju</label><input id="name" name="name" value="${demo.name}" required maxlength="120" autocomplete="off">` : ''}${key === 'errors' ? '<label for="scenario">Vrsta zahtjeva</label><select id="scenario"><option value="missing">404 · Nepostojeća datoteka</option><option value="method">405 · POST na statičku datoteku</option><option value="forbidden">403 · Izlazak iz web-korijena</option></select>' : ''}<label>Zahtjev</label><code id="request" class="request"></code><button type="submit">Pošalji ${key === 'errors' ? 'zahtjev' : demo.method}</button></form><div><div class="result-title"><span>Odgovor poslužitelja</span><span id="status" class="badge"></span></div><pre id="result" aria-live="polite">Pošalji zahtjev za prikaz rezultata.</pre></div></div>`;
 const form = panel.querySelector('form');
 function request() {
 let method = demo.method, url = demo.url, body;
 const name = form.elements.name?.value;
 if (key === 'get') url += '?' + new URLSearchParams({name});
 if (key === 'post') body = new URLSearchParams({name});
 if (key === 'errors') {
 const scenario = document.querySelector('#scenario').value;
 if (scenario === 'method') { method = 'POST'; url = '/primjer.txt'; }
 if (scenario === 'forbidden') url = '/..%2Fizvan-korijena.txt';
 }
 return {method, url, body};
 }
 function update() {const r = request(); document.querySelector('#request').textContent = `${r.method} ${r.url}${r.body ? '\nContent-Type: application/x-www-form-urlencoded\n\n' + r.body : ''}`;}
 form.addEventListener('input', update); update();
 form.addEventListener('submit', async event => {
 event.preventDefault(); const r = request(); const button = form.querySelector('button');
 const status = document.querySelector('#status'), result = document.querySelector('#result');
 button.disabled = true; status.className = 'badge'; status.textContent = 'Slanje…'; result.textContent = 'Čekanje odgovora poslužitelja…';
 try {
 const response = await fetch(r.url, {method:r.method, ...(r.body ? {body:r.body} : {}), cache:'no-store', signal:AbortSignal.timeout(15000)});
 const body = await response.text();
 if (current !== generation) return;
 const headers = ['content-type','content-length','connection','allow'].filter(h => response.headers.has(h)).map(h => `${h}: ${response.headers.get(h)}`).join('\n');
 result.textContent = `HTTP ${response.status} ${response.statusText}\n${headers}\n\n${r.method === 'HEAD' ? '(Bez tijela odgovora · 0 bajtova)' : body}`;
 status.textContent = `${response.status} ${response.statusText}`;
 status.className = 'badge ' + (response.ok ? 'success' : 'error');
 } catch(error) {if (current !== generation) return; status.textContent = 'Zahtjev nije uspio'; status.className = 'badge error'; result.textContent = `Nije moguće dohvatiti odgovor. Provjeri je li poslužitelj pokrenut.\n${error.message}`;}
 finally {button.disabled = false;}
 });
}
document.querySelectorAll('[data-demo]').forEach(button => button.addEventListener('click', () => render(button.dataset.demo)));
render('overview');
