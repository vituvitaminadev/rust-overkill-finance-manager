# TLS autoassinado, com o certificado do Anfitrião fixado no Convite

O app trafega senhas e dados financeiros pela rede local, sem nuvem nem autoridade certificadora. O Anfitrião gera um certificado autoassinado e o app fixa a impressão digital dele no momento em que o Convite é aceito, recusando depois qualquer servidor com outro certificado. Escolhemos isso em vez de HTTP puro porque qualquer dispositivo no mesmo Wi-Fi poderia ler o tráfego, e porque adotar TLS mais tarde obrigaria todos os membros a refazer o pareamento.

## Consequences

- Trocar o certificado do Anfitrião (inclusive ao migrar de máquina) invalida o pareamento de todos os apps.
- Não há acesso pela internet: o Anfitrião só é alcançável na rede local, descoberto por mDNS.
- No primeiro contato não há o que fixar ainda, então a Cópia aceita o certificado apresentado e mostra o Código de segurança dele; a pessoa o compara com o exibido no Anfitrião antes de digitar o Convite. Sem essa conferência, alguém na mesma rede poderia se passar pelo Anfitrião durante o pareamento.
- O certificado fica guardado no banco da residência, e não na instalação do app.
