function getCookie(name) {
    let c = document.cookie.match(`(?:(?:^|.*; *)${name} *= *([^;]*).*$)|^.*$`)[1];
    if (c) return decodeURIComponent(c);
}

function formatSeconds(time) {
    const hours = Math.trunc(time / 3600);
    const minutes = Math.trunc(time / 60) % 60;

    return time >= 3600 ? `${hours}h${minutes.toString().padStart(2, '0')}` : `${minutes}m`;
}

const ws = new WebSocket(`${window.location.origin.replace('http', 'ws')}/ws?sid=${getCookie('id')}`);

const challengesContainer = document.getElementById('challenges-ctn');
const challenges = {};

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
        actionsQueuedStart.textContent = 'queued start';

        const actionsQueuedRestart = document.createElement('div');
        actions.appendChild(actionsQueuedRestart);
        actionsQueuedRestart.classList.add('actions-queued-restart');
        actionsQueuedRestart.textContent = 'queued restart';

        const actionsQueuedStop = document.createElement('div');
        actions.appendChild(actionsQueuedStop);
        actionsQueuedStop.classList.add('actions-queued-stop');
        actionsQueuedStop.textContent = 'queued stop';
    }

    card.onclick = e => {
        if(e.target.nodeName !== 'BUTTON') return;

        const action = e.target.getAttribute('data-action');
        if(action == null) return;

        switch(action) {
            case 'start':
                ws.send(JSON.stringify({'type': 'challenge_start', 'id': challenge.id}))
                break;
            case 'stop':
                ws.send(JSON.stringify({'type': 'challenge_stop', 'id': challenge.id}))
                break;
            case 'restart':
                ws.send(JSON.stringify({'type': 'challenge_restart', 'id': challenge.id}))
                break;
            case 'extend':
                ws.send(JSON.stringify({'type': 'challenge_extend', 'id': challenge.id}))
                break;
            default:
                return;
        }
    };

    challenge.dom = card;
}

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
            break;
    }
};