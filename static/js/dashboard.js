function getCookie(name) {
    let c = document.cookie.match(`(?:(?:^|.*; *)${name} *= *([^;]*).*$)|^.*$`)[1];
    if (c) return decodeURIComponent(c);
}

function formatSeconds(time) {
    const hours = Math.trunc(time / 3600);
    const minutes = Math.trunc(time / 60) % 60;
    const seconds = time % 60;

    let formatted = '';
    if(time >= 3600) formatted += `${hours}h `;
    if(time >= 60) formatted += `${minutes}m `;
    return formatted + `${seconds}s`;
}

function formatRemainingTime(stop_time) {
    if(!stop_time) return '';
    if(Date.now() > stop_time) return '⏱️ Défi expiré...';
    return '⏱️ ' + formatSeconds(Math.ceil((stop_time - Date.now()) / 1000));
}

const challengesContainer = document.getElementById('challenges-ctn');
const challenges = {};

let ws;

function connectWS() {
    ws = new WebSocket(`${window.location.origin.replace('http', 'ws')}/ws?sid=${getCookie('id')}`);

    ws.onmessage = e => {
        const msg = JSON.parse(e.data);

        switch(msg.type) {
            case 'challenge_listing':
                for(let id of Object.keys(msg.challenges).toSorted()) {
                    challenges[id] = msg.challenges[id];
                    loadChallengeDOM(challenges[id]);
                }
                break;
            case 'challenge_state_change':
                const challenge = challenges[msg.id];
                challenge.state = msg.state;
                challenge.dom.setAttribute('data-state', msg.state);
                if(msg.details) {
                    challenge.details = msg.details;
                    challenge.dom.querySelector('.instance-details').textContent = msg.details;
                }
                if(msg.stop_time) {
                    challenge.stop_time = msg.stop_time;
                    challenge.dom.querySelector('.ttl').textContent = formatRemainingTime(msg.stop_time);
                }
                break;
            case 'message':
                const text = document.createElement('span');
                text.innerHTML = msg.contents;
                Toastify({
                    node: text,
                    className: msg.severity,
                    close: msg.severity === 'error',
                    duration: msg.severity === 'error' ? -1 : 2500,
                    position: 'right',
                    gravity: 'bottom'
                }).showToast();
                break;
        }
    };

    ws.onclose = _ => {
        for(let key of Object.keys(challenges)) delete challenges[key];
        challengesContainer.innerHTML = '';
        Toastify({
            text: 'La connexion avec le serveur a été perdue.\nReconnexion dans 5 secondes...',
            className: 'warning',
            duration: 5000,
            position: 'right',
            gravity: 'bottom'
        }).showToast();
        setTimeout(connectWS, 5000);
    }
}

connectWS();

function loadChallengeDOM(challenge) {
    const card = document.createElement('div');
    challengesContainer.appendChild(card);
    card.classList.add('challenge-card');
    card.setAttribute('data-cid', challenge.id);
    card.setAttribute('data-state', challenge.state);

    const details = document.createElement('div');
    card.appendChild(details);
    details.classList.add('details');

    {
        const title = document.createElement('h3');
        details.appendChild(title);
        title.textContent = challenge.name;

        if (challenge.description) {
            const description = document.createElement('p');
            details.appendChild(description);
            description.textContent = challenge.description;
        }
    }

    const actions = document.createElement('div');
    card.appendChild(actions);
    actions.classList.add('actions');

    {
        const actionsStopped = document.createElement('div');
        actions.appendChild(actionsStopped);
        actionsStopped.classList.add('actions-stopped');

        {
            const startButton = document.createElement('button');
            actionsStopped.appendChild(startButton);
            startButton.textContent = 'Démarrer';
            startButton.setAttribute('data-action', 'start');
        }

        const actionsRunning = document.createElement('div');
        actions.appendChild(actionsRunning);
        actionsRunning.classList.add('actions-running');

        {
            const detailsText = document.createElement('pre')
            actionsRunning.appendChild(detailsText);
            detailsText.classList.add('instance-details');
            detailsText.textContent = challenge.details;

            const ttlText = document.createElement('p');
            actionsRunning.appendChild(ttlText);
            ttlText.classList.add('ttl');
            ttlText.textContent = formatRemainingTime(challenge.stop_time);

            const stopButton = document.createElement('button');
            actionsRunning.appendChild(stopButton);
            stopButton.textContent = 'Arrêter';
            stopButton.setAttribute('data-action', 'stop');

            const restartButton = document.createElement('button');
            actionsRunning.appendChild(restartButton);
            restartButton.textContent = 'Redémarrer';
            restartButton.setAttribute('data-action', 'restart');

            const extendButton = document.createElement('button');
            actionsRunning.appendChild(extendButton);
            extendButton.textContent = 'Étendre';
            extendButton.setAttribute('data-action', 'extend');
        }

        const actionsQueuedStart = document.createElement('div');
        actions.appendChild(actionsQueuedStart);
        actionsQueuedStart.classList.add('actions-queued-start');
        actionsQueuedStart.textContent = 'En attente du démarrage...';

        const actionsQueuedRestart = document.createElement('div');
        actions.appendChild(actionsQueuedRestart);
        actionsQueuedRestart.classList.add('actions-queued-restart');
        actionsQueuedRestart.textContent = 'En attente du redémarrage...';

        const actionsQueuedStop = document.createElement('div');
        actions.appendChild(actionsQueuedStop);
        actionsQueuedStop.classList.add('actions-queued-stop');
        actionsQueuedStop.textContent = 'En attente de l\'arrêt...';
    }

    card.onclick = e => {
        if(e.target.nodeName !== 'BUTTON') return;

        const action = e.target.getAttribute('data-action');
        if(action == null) return;

        switch(action) {
            case 'start':
            case 'stop':
            case 'restart':
            case 'extend':
                ws.send(JSON.stringify({'type': 'challenge_action', 'id': challenge.id, 'action': action}));
                break;
            default:
                return;
        }
    };

    challenge.dom = card;
}

setInterval(() => {
    for(let id of Object.keys(challenges)) {
        const challenge = challenges[id];
        if(challenge.state === 'running') {
            challenge.dom.querySelector('.ttl').textContent = formatRemainingTime(challenge.stop_time);
        }
    }
}, 1000);