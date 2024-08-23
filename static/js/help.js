for(let qaTitle of document.querySelectorAll('.qa>h2')) {
    qaTitle.onclick = _ => {
        const card = qaTitle.parentElement;
        if(card.getAttribute('data-open') === 'false') {
            card.setAttribute('data-open', 'true');
        } else {
            card.setAttribute('data-open', 'false');
        }
    };
}