const Cookie = {
    get: name => {
        let c = document.cookie.match(`(?:(?:^|.*; *)${name} *= *([^;]*).*$)|^.*$`)[1]
        if (c) return decodeURIComponent(c)
    }
}

const sid = Cookie.get('id');
const ws = new WebSocket(`${window.location.origin.replace('http', 'ws')}/ws?sid=${sid}`);

ws.onmessage = e => {
    console.log(e);
};

// $('button.spawn').on('click', e => {
//     let challengeCard = $(e.target).parents('.challenge-card')[0];
//     let challengeId = challengeCard.getAttribute('data-cid');
//
//     fetch('challenges/spawn', {
//         method: 'POST',
//         headers: {
//             'Content-Type': 'application/json'
//         },
//         body: JSON.stringify({'cid': challengeId})
//     }).then(r => console.log(r));
// });
//
// $('button.destroy').on('click', e => {
//     let challengeCard = $(e.target).parents('.challenge-card')[0];
//     let challengeId = challengeCard.getAttribute('data-cid');
//
//     fetch('challenges/destroy', {
//         method: 'POST',
//         headers: {
//             'Content-Type': 'application/json'
//         },
//         body: JSON.stringify({'cid': challengeId})
//     }).then(r => console.log(r));
// });