function getCookie(name) {
    let c = document.cookie.match(`(?:(?:^|.*; *)${name} *= *([^;]*).*$)|^.*$`)[1];
    if (c) return decodeURIComponent(c);
}

function formatSeconds(time) {
    const hours = Math.trunc(time / 3600);
    const minutes = Math.trunc(time / 60) % 60;

    return time >= 3600 ? `${hours}h${minutes.toString().padStart(2, '0')}` : `${minutes}m`;
}

const challengesContainer = document.getElementById('challenges-ctn');

const sid = getCookie('id');
const ws = new WebSocket(`${window.location.origin.replace('http', 'ws')}/ws?sid=${sid}`);

const challenges = {};

function loadChallengeDOM(challenge) {
    const card = document.createElement('div');
    challengesContainer.appendChild(card);
    card.classList.add('challenge-card');
    card.setAttribute('data-cid', challenge.id);

    const details = document.createElement('div');
    card.appendChild(details);
    details.classList.add('details');

        const title = document.createElement('h3');
        details.appendChild(title);
        title.textContent = challenge.name;

        if(challenge.description) {
            const description = document.createElement('p');
            details.appendChild(description);
            description.textContent = challenge.description;
        }

    const actions = document.createElement('div');
    card.appendChild(actions);
    actions.classList.add('actions');
    actions.setAttribute('data-state', challenge.state.type);

        const actionsNone = document.createElement('div');
        actions.appendChild(actionsNone);
        actionsNone.classList.add('actions-none');

            const startButton = document.createElement('button');
            actionsNone.appendChild(startButton);
            startButton.textContent = 'Démarrer';
            startButton.setAttribute('data-action', 'start');

        const actionsDeployed = document.createElement('div');
        actions.appendChild(actionsDeployed);
        actionsDeployed.classList.add('actions-deployed');

            const stopButton = document.createElement('button');
            actionsDeployed.appendChild(stopButton);
            stopButton.textContent = 'Arrêter';
            stopButton.setAttribute('data-action', 'stop');

            const restartButton = document.createElement('button');
            actionsDeployed.appendChild(restartButton);
            restartButton.textContent = 'Redémarrer';
            restartButton.setAttribute('data-action', 'restart');

            const extendButton = document.createElement('button');
            actionsDeployed.appendChild(extendButton);
            extendButton.textContent = 'Étendre';
            extendButton.setAttribute('data-action', 'extend');

        const actionsQueued = document.createElement('div');
        actions.appendChild(actionsQueued);
        actionsQueued.classList.add('actions-queued');

        const actionsDeploying = document.createElement('div');
        actions.appendChild(actionsDeploying);
        actionsDeploying.classList.add('actions-deploying');

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
    }

    ws.send(JSON.stringify({'type': 'challenge_start', 'id': 'my-challenge-1'}))
};