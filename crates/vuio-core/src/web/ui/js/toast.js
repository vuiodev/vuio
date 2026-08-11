function showToast(message, type = 'info', onClick = null) {
    let container = document.getElementById('toast-container');
    if (!container) {
        container = document.createElement('div');
        container.id = 'toast-container';
        container.style.position = 'fixed';
        container.style.bottom = '1.5rem';
        container.style.right = '1.5rem';
        container.style.display = 'flex';
        container.style.flexDirection = 'column';
        container.style.gap = '0.5rem';
        container.style.zIndex = '9999';
        document.body.appendChild(container);
    }

    const toast = document.createElement('div');
    toast.style.background = '#1e293b';
    toast.style.border = '1px solid ' + (type === 'error' ? '#ef4444' : type === 'success' ? '#10b981' : 'var(--accent-color)');
    toast.style.color = '#fff';
    toast.style.padding = '0.8rem 1.4rem';
    toast.style.borderRadius = '12px';
    toast.style.fontSize = '0.9rem';
    toast.style.fontWeight = '600';
    toast.style.boxShadow = '0 15px 30px rgba(0,0,0,0.3)';
    toast.style.opacity = '0';
    toast.style.transform = 'translateY(15px)';
    toast.style.transition = 'all 0.3s cubic-bezier(0.16, 1, 0.3, 1)';
    
    toast.textContent = message;
    if (onClick) {
        toast.style.cursor = 'pointer';
        toast.style.textDecoration = 'underline';
        toast.addEventListener('click', () => {
            toast.remove();
            onClick();
        });
    }
    container.appendChild(toast);
    
    setTimeout(() => {
        toast.style.opacity = '1';
        toast.style.transform = 'translateY(0)';
    }, 15);
    
    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateY(-15px)';
        setTimeout(() => {
            toast.remove();
        }, 300);
    }, 4500);
}
