# Sincronização por versão de registro, com Anfitrião fixo como fonte da verdade

Cada app mantém uma cópia completa dos dados da residência e permite criar, editar e excluir lançamentos offline; ao alcançar o Anfitrião, envia suas alterações informando a versão de que partiu, e o Anfitrião recusa as que partiram de uma versão desatualizada, gerando um Conflito que a pessoa resolve escolhendo entre a sua versão e a do Anfitrião. Escolhemos isso porque resolver divergências manualmente, como num merge do git, é um requisito do produto, e porque um único Anfitrião por residência (a máquina onde ela foi criada) elimina a possibilidade de duas histórias concorrentes.

## Considered Options

- **CRDT (cr-sqlite e similares)**: rejeitado porque faz merge automático campo a campo e nunca apresenta a escolha ao usuário.
- **Qualquer cópia pode virar Anfitrião**: rejeitado porque dois Anfitriões simultâneos criam histórias divergentes sem fonte da verdade.
- **Cliente apenas online**: rejeitado porque o app precisa funcionar com o PC anfitrião desligado.

## Consequences

- Conflito é por lançamento inteiro, e editar contra excluir também conflita; exclusões são marcadas em vez de removidas.
- Só lançamentos podem ser alterados offline. Categorias, membros, convites, papéis, recorrências e parcelamentos exigem conexão com o Anfitrião, de modo que Conflito só existe em Lançamento.
- Trocar a máquina anfitriã é uma operação explícita de exportar e importar, não um failover.
- App e Anfitrião com números de protocolo diferentes suspendem a sincronização até que ambos atualizem; o app segue funcionando offline.
